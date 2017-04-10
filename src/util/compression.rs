use std::ptr;
use std::ffi::{CStr, CString};
use std::io::{self, Write};
use std::str::FromStr;

use libc;
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

#[derive(Clone, Debug, Copy, Eq, PartialEq)]
pub enum CompressionMethod {
    Deflate, // Standardized
    Brotli, // Good speed and ratio
    Lzma, // Very good ratio, slow
    Lz4 // Very fast, low ratio
}
serde_impl!(CompressionMethod(u8) {
    Deflate => 0,
    Brotli => 1,
    Lzma => 2,
    Lz4 => 3
});


#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Compression {
    method: CompressionMethod,
    level: u8
}
impl Default for Compression {
    fn default() -> Self {
        Compression { method: CompressionMethod::Brotli, level: 3 }
    }
}
serde_impl!(Compression(u64) {
    method: CompressionMethod => 0,
    level: u8 => 1
});


impl Compression {
    #[inline]
    pub fn to_string(&self) -> String {
        format!("{}/{}", self.name(), self.level)
    }

    pub fn from_string(name: &str) -> Result<Self, CompressionError> {
        let (name, level) = if let Some(pos) = name.find('/') {
            let level = try!(u8::from_str(&name[pos+1..]).map_err(|_| CompressionError::UnsupportedCodec(name.to_string())));
            let name = &name[..pos];
            (name, level)
        } else {
            (name, 5)
        };
        let method = match name {
            "deflate" | "zlib" | "gzip" => CompressionMethod::Deflate,
            "brotli" => CompressionMethod::Brotli,
            "lzma" | "lzma2" | "xz" => CompressionMethod::Lzma,
            "lz4" => CompressionMethod::Lz4,
            _ => return Err(CompressionError::UnsupportedCodec(name.to_string()))
        };
        Ok(Compression { method: method, level: level })
    }

    pub fn name(&self) -> &'static str {
        match self.method {
            CompressionMethod::Deflate => "deflate",
            CompressionMethod::Brotli => "brotli",
            CompressionMethod::Lzma => "lzma",
            CompressionMethod::Lz4 => "lz4",
        }
    }

    fn codec(&self) -> Result<*mut SquashCodec, CompressionError> {
        let name = CString::new(self.name().as_bytes()).unwrap();
        let codec = unsafe { squash_get_codec(name.as_ptr()) };
        if codec.is_null() {
            return Err(CompressionError::InitializeCodec)
        }
        Ok(codec)
    }

    #[inline]
    pub fn level(&self) -> u8 {
        self.level
    }

    fn options(&self) -> Result<*mut SquashOptions, CompressionError> {
        let codec = try!(self.codec());
        let options = unsafe { squash_options_new(codec, ptr::null::<()>()) };
        if options.is_null() {
            return Err(CompressionError::InitializeOptions)
        }
        let option = CString::new("level");
        let value = CString::new(format!("{}", self.level));
        let res = unsafe { squash_options_parse_option(
            options,
            option.unwrap().as_ptr(),
            value.unwrap().as_ptr()
        )};
        if res != SQUASH_OK {
            //panic!(unsafe { CStr::from_ptr(squash_status_to_string(res)).to_str().unwrap() });
            return Err(CompressionError::InitializeOptions)
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

    pub fn compress_stream(&self) -> Result<CompressionStream, CompressionError> {
        let codec = try!(self.codec());
        let options = try!(self.options());
        let stream = unsafe { squash_stream_new_with_options(
            codec, SQUASH_STREAM_COMPRESS, options
        ) };
        if stream.is_null() {
            return Err(CompressionError::InitializeStream);
        }
        Ok(CompressionStream::new(stream))
    }

    pub fn decompress_stream(&self) -> Result<CompressionStream, CompressionError> {
        let codec = try!(self.codec());
        let stream = unsafe { squash_stream_new(
            codec, SQUASH_STREAM_DECOMPRESS, ptr::null::<()>()
        ) };
        if stream.is_null() {
            return Err(CompressionError::InitializeStream);
        }
        Ok(CompressionStream::new(stream))
    }
}


pub struct CompressionStream {
    stream: *mut SquashStream,
    buffer: [u8; 16*1024]
}

impl CompressionStream {
    #[inline]
    fn new(stream: *mut SquashStream) -> Self {
        CompressionStream {
            stream: stream,
            buffer: [0; 16*1024]
        }
    }

    pub fn process<W: Write>(&mut self, input: &[u8], output: &mut W) -> Result<(), CompressionError> {
        let stream = unsafe { &mut (*self.stream) };
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
        let stream = unsafe { &mut (*self.stream) };
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

impl Drop for CompressionStream {
    fn drop(&mut self) {
        unsafe { squash_object_unref(self.stream as *mut libc::c_void); }
    }
}
