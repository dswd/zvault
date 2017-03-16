use std::ptr;
use std::ffi::{CStr, CString};
use std::io::{self, Write};
use std::str::FromStr;

use squash::*;


quick_error!{
    #[derive(Debug)]
    pub enum CompressionError {
        UnsupportedCodec(name: String) {
            description("Unsupported codec")
            display("Unsupported codec: {}", name)
        }
        InitializeCodec {
            description("Failed to initialize codec")
        }
        InitializeOptions {
            description("Failed to set codec options")
        }
        InitializeStream {
            description("Failed to create stream")
        }
        Operation(reason: &'static str) {
            description("Operation failed")
            display("Operation failed: {}", reason)
        }
        Output(err: io::Error) {
            from()
            cause(err)
            description("Failed to write to output")
        }
    }
}


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
    pub fn to_string(&self) -> String {
        if let Some(level) = self.level() {
            format!("{}/{}", self.name(), level)
        } else {
            self.name().to_string()
        }
    }

    #[inline]
    pub fn from_string(name: &str) -> Result<Self, CompressionError> {
        let (name, level) = if let Some(pos) = name.find('/') {
            let level = try!(u8::from_str(&name[pos+1..]).map_err(|_| CompressionError::UnsupportedCodec(name.to_string())));
            let name = &name[..pos];
            (name, level)
        } else {
            (name, 5)
        };
        match name {
            "snappy" => Ok(Compression::Snappy(())),
            "zstd" => Ok(Compression::ZStd(level)),
            "deflate" | "zlib" | "gzip" => Ok(Compression::Deflate(level)),
            "brotli" => Ok(Compression::Brotli(level)),
            "lzma2" => Ok(Compression::Lzma2(level)),
            _ => Err(CompressionError::UnsupportedCodec(name.to_string()))
        }
    }

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
    fn codec(&self) -> Result<*mut SquashCodec, CompressionError> {
        let name = CString::new(self.name().as_bytes()).unwrap();
        let codec = unsafe { squash_get_codec(name.as_ptr()) };
        if codec.is_null() {
            return Err(CompressionError::InitializeCodec)
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

    fn options(&self) -> Result<*mut SquashOptions, CompressionError> {
        let codec = try!(self.codec());
        let options = unsafe { squash_options_new(codec, ptr::null::<()>()) };
        if let Some(level) = self.level() {
            if options.is_null() {
                return Err(CompressionError::InitializeOptions)
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
                return Err(CompressionError::InitializeOptions)
            }
        }
        Ok(options)
    }

    #[inline]
    fn error(code: SquashStatus) -> CompressionError {
        CompressionError::Operation(unsafe { CStr::from_ptr(squash_status_to_string(code)).to_str().unwrap() })
    }

    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
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

    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
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
    pub fn compress_stream(&self) -> Result<CompressionStream, CompressionError> {
        let codec = try!(self.codec());
        let options = try!(self.options());
        let stream = unsafe { squash_stream_new_with_options(
            codec, SQUASH_STREAM_COMPRESS, options
        ) };
        if stream.is_null() {
            return Err(CompressionError::InitializeStream);
        }
        Ok(CompressionStream::new(unsafe { Box::from_raw(stream) }))
    }

    #[inline]
    pub fn decompress_stream(&self) -> Result<CompressionStream, CompressionError> {
        let codec = try!(self.codec());
        let stream = unsafe { squash_stream_new(
            codec, SQUASH_STREAM_DECOMPRESS, ptr::null::<()>()
        ) };
        if stream.is_null() {
            return Err(CompressionError::InitializeStream);
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

    pub fn process<W: Write>(&mut self, input: &[u8], output: &mut W) -> Result<(), CompressionError> {
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
            try!(output.write_all(&self.buffer[..output_size]));
            if res != SQUASH_PROCESSING {
                break
            }
        }
        Ok(())
    }

    pub fn finish<W: Write>(mut self, output: &mut W) -> Result<(), CompressionError> {
        let mut stream = &mut *self.stream;
        loop {
            stream.next_out = self.buffer.as_mut_ptr();
            stream.avail_out = self.buffer.len();
            let res = unsafe { squash_stream_finish(stream) };
            if res < 0 {
                return Err(Compression::error(res))
            }
            let output_size = self.buffer.len() - stream.avail_out;
            try!(output.write_all(&self.buffer[..output_size]));
            if res != SQUASH_PROCESSING {
                break
            }
        }
        Ok(())
    }
}
