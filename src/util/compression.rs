use std::ptr;
use std::ffi::{CStr, CString};
use std::io::Write;

use squash::*;


#[derive(Clone, Debug)]
pub enum Compression {
    Snappy(()),
    Deflate(u8),
    Brotli(u8),
    Lzma2(u8),
    ZStd(u8)
}
serde_impl!(Compression(u64) {
    Snappy(()) => 0,
    Deflate(u8) => 1,
    Brotli(u8) => 2,
    Lzma2(u8) => 3,
    ZStd(u8) => 4
});


impl Compression {
    #[inline]
    pub fn name(&self) -> &'static str {
        match *self {
            Compression::Snappy(_) => "snappy",
            Compression::Deflate(_) => "deflate",
            Compression::Brotli(_) => "brotli",
            Compression::Lzma2(_) => "lzma2",
            Compression::ZStd(_) => "zstd",
        }
    }

    #[inline]
    fn codec(&self) -> Result<*mut SquashCodec, &'static str> {
        let name = CString::new(self.name().as_bytes()).unwrap();
        let codec = unsafe { squash_get_codec(name.as_ptr()) };
        if codec.is_null() {
            return Err("Unsupported algorithm")
        }
        Ok(codec)
    }

    #[inline]
    pub fn level(&self) -> Option<u8> {
        match *self {
            Compression::Snappy(_) => None,
            Compression::Deflate(lvl) |
            Compression::Brotli(lvl) |
            Compression::ZStd(lvl) |
            Compression::Lzma2(lvl) => Some(lvl),
        }
    }

    fn options(&self) -> Result<*mut SquashOptions, &'static str> {
        let codec = try!(self.codec());
        let options = unsafe { squash_options_new(codec, ptr::null::<()>()) };
        if let Some(level) = self.level() {
            if options.is_null() {
                return Err("Algorithm does not support a level")
            }
            let option = CString::new("level");
            let value = CString::new(format!("{}", level));
            let res = unsafe { squash_options_parse_option(
                options,
                option.unwrap().as_ptr(),
                value.unwrap().as_ptr()
            )};
            if res != SQUASH_OK {
                //panic!(unsafe { CStr::from_ptr(squash_status_to_string(res)).to_str().unwrap() });
                return Err("Failed to set compression level")
            }
        }
        Ok(options)
    }

    #[inline]
    fn error(code: SquashStatus) -> &'static str {
        unsafe { CStr::from_ptr(squash_status_to_string(code)).to_str().unwrap() }
    }

    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>, &'static str> {
        let codec = try!(self.codec());
        let options = try!(self.options());
        let mut size = data.len() * 2 + 500;
        // The following does not work for all codecs
        /*unsafe { squash_codec_get_max_compressed_size(
            codec,
            data.len() as usize
        )};*/
        let mut buf = Vec::with_capacity(size as usize);
        let res = unsafe { squash_codec_compress_with_options(
            codec,
            &mut size,
            buf.as_mut_ptr(),
            data.len(),
            data.as_ptr(),
            options)
        };
        if res != SQUASH_OK {
    	    println!("{:?}", data);
            println!("{}, {}", data.len(), size);
            return Err(Self::error(res))
        }
        unsafe { buf.set_len(size) };
        Ok(buf)
    }

    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, &'static str> {
        let codec = try!(self.codec());
        let mut size = unsafe { squash_codec_get_uncompressed_size(
            codec,
            data.len(),
            data.as_ptr()
        )};
        if size == 0 {
            size = 100 * data.len();
        }
        let mut buf = Vec::with_capacity(size);
        let res = unsafe { squash_codec_decompress(
            codec,
            &mut size,
            buf.as_mut_ptr(),
            data.len(),
            data.as_ptr(),
            ptr::null_mut::<()>())
        };
        if res != SQUASH_OK {
            return Err(Self::error(res))
        }
        unsafe { buf.set_len(size) };
        Ok(buf)
    }

    #[inline]
    pub fn compress_stream(&self) -> Result<CompressionStream, &'static str> {
        let codec = try!(self.codec());
        let options = try!(self.options());
        let stream = unsafe { squash_stream_new_with_options(
            codec, SQUASH_STREAM_COMPRESS, options
        ) };
        if stream.is_null() {
            return Err("Failed to create stream");
        }
        Ok(CompressionStream::new(unsafe { Box::from_raw(stream) }))
    }

    #[inline]
    pub fn decompress_stream(&self) -> Result<CompressionStream, &'static str> {
        let codec = try!(self.codec());
        let stream = unsafe { squash_stream_new(
            codec, SQUASH_STREAM_DECOMPRESS, ptr::null::<()>()
        ) };
        if stream.is_null() {
            return Err("Failed to create stream");
        }
        Ok(CompressionStream::new(unsafe { Box::from_raw(stream) }))
    }
}


pub struct CompressionStream {
    stream: Box<SquashStream>,
    buffer: [u8; 16*1024]
}

impl CompressionStream {
    #[inline]
    fn new(stream: Box<SquashStream>) -> Self {
        CompressionStream {
            stream: stream,
            buffer: [0; 16*1024]
        }
    }

    pub fn process<W: Write>(&mut self, input: &[u8], output: &mut W) -> Result<(), &'static str> {
        let mut stream = &mut *self.stream;
        stream.next_in = input.as_ptr();
        stream.avail_in = input.len();
        loop {
            stream.next_out = self.buffer.as_mut_ptr();
            stream.avail_out = self.buffer.len();
            let res = unsafe { squash_stream_process(stream) };
            if res < 0 {
                return Err(Compression::error(res))
            }
            let output_size = self.buffer.len() - stream.avail_out;
            try!(output.write_all(&self.buffer[..output_size]).map_err(|_| "Failed to write to output"));
            if res != SQUASH_PROCESSING {
                break
            }
        }
        Ok(())
    }

    pub fn finish<W: Write>(mut self, output: &mut W) -> Result<(), &'static str> {
        let mut stream = &mut *self.stream;
        loop {
            stream.next_out = self.buffer.as_mut_ptr();
            stream.avail_out = self.buffer.len();
            let res = unsafe { squash_stream_finish(stream) };
            if res < 0 {
                return Err(Compression::error(res))
            }
            let output_size = self.buffer.len() - stream.avail_out;
            try!(output.write_all(&self.buffer[..output_size]).map_err(|_| "Failed to write to output"));
            if res != SQUASH_PROCESSING {
                break
            }
        }
        Ok(())
    }
}
