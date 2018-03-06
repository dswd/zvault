use std::ptr;
use std::ffi::{CStr, CString};
use std::io::{self, Write};
use std::str::FromStr;

use squash::*;


quick_error!{
    #[derive(Debug)]
    pub enum CompressionError {
        UnsupportedCodec(name: String) {
            description(tr!("Unsupported codec"))
            display("{}", tr_format!("Unsupported codec: {}", name))
        }
        InitializeCodec {
            description(tr!("Failed to initialize codec"))
        }
        InitializeOptions {
            description(tr!("Failed to set codec options"))
        }
        InitializeStream {
            description(tr!("Failed to create stream"))
        }
        Operation(reason: &'static str) {
            description(tr!("Operation failed"))
            display("{}", tr_format!("Operation failed: {}", reason))
        }
        Output(err: io::Error) {
            from()
            cause(err)
            description(tr!("Failed to write to output"))
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
        Compression {
            method: CompressionMethod::Brotli,
            level: 3
        }
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
            let level = try!(u8::from_str(&name[pos + 1..]).map_err(|_| {
                CompressionError::UnsupportedCodec(name.to_string())
            }));
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
            _ => return Err(CompressionError::UnsupportedCodec(name.to_string())),
        };
        Ok(Compression {
            method,
            level
        })
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
            return Err(CompressionError::InitializeCodec);
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
            return Err(CompressionError::InitializeOptions);
        }
        let option = CString::new("level");
        let value = CString::new(format!("{}", self.level));
        let res = unsafe {
            squash_options_parse_option(options, option.unwrap().as_ptr(), value.unwrap().as_ptr())
        };
        if res != SQUASH_OK {
            //panic!(unsafe { CStr::from_ptr(squash_status_to_string(res)).to_str().unwrap() });
            return Err(CompressionError::InitializeOptions);
        }
        Ok(options)
    }

    #[inline]
    fn error(code: SquashStatus) -> CompressionError {
        CompressionError::Operation(unsafe {
            CStr::from_ptr(squash_status_to_string(code))
                .to_str()
                .unwrap()
        })
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
        let res = unsafe {
            squash_codec_compress_with_options(
                codec,
                &mut size,
                buf.as_mut_ptr(),
                data.len(),
                data.as_ptr(),
                options
            )
        };
        if res != SQUASH_OK {
            println!("{:?}", data);
            println!("{}, {}", data.len(), size);
            return Err(Self::error(res));
        }
        unsafe { buf.set_len(size) };
        Ok(buf)
    }

    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let codec = try!(self.codec());
        let mut size =
            unsafe { squash_codec_get_uncompressed_size(codec, data.len(), data.as_ptr()) };
        if size == 0 {
            size = 100 * data.len();
        }
        let mut buf = Vec::with_capacity(size);
        let res = unsafe {
            squash_codec_decompress(
                codec,
                &mut size,
                buf.as_mut_ptr(),
                data.len(),
                data.as_ptr(),
                ptr::null_mut::<()>()
            )
        };
        if res != SQUASH_OK {
            return Err(Self::error(res));
        }
        unsafe { buf.set_len(size) };
        Ok(buf)
    }

    pub fn compress_stream(&self) -> Result<CompressionStream, CompressionError> {
        let codec = try!(self.codec());
        let options = try!(self.options());
        let stream =
            unsafe { squash_stream_new_with_options(codec, SQUASH_STREAM_COMPRESS, options) };
        if stream.is_null() {
            return Err(CompressionError::InitializeStream);
        }
        Ok(CompressionStream::new(stream))
    }

    pub fn decompress_stream(&self) -> Result<CompressionStream, CompressionError> {
        let codec = try!(self.codec());
        let stream =
            unsafe { squash_stream_new(codec, SQUASH_STREAM_DECOMPRESS, ptr::null::<()>()) };
        if stream.is_null() {
            return Err(CompressionError::InitializeStream);
        }
        Ok(CompressionStream::new(stream))
    }
}


pub struct CompressionStream {
    stream: *mut SquashStream,
    buffer: [u8; 16 * 1024]
}

impl CompressionStream {
    #[inline]
    fn new(stream: *mut SquashStream) -> Self {
        CompressionStream {
            stream,
            buffer: [0; 16 * 1024]
        }
    }

    pub fn process<W: Write>(
        &mut self,
        input: &[u8],
        output: &mut W,
    ) -> Result<(), CompressionError> {
        let stream = unsafe { &mut (*self.stream) };
        stream.next_in = input.as_ptr();
        stream.avail_in = input.len();
        loop {
            stream.next_out = self.buffer.as_mut_ptr();
            stream.avail_out = self.buffer.len();
            let res = unsafe { squash_stream_process(stream) };
            if res < 0 {
                return Err(Compression::error(res));
            }
            let output_size = self.buffer.len() - stream.avail_out;
            try!(output.write_all(&self.buffer[..output_size]));
            if res != SQUASH_PROCESSING {
                break;
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
                return Err(Compression::error(res));
            }
            let output_size = self.buffer.len() - stream.avail_out;
            try!(output.write_all(&self.buffer[..output_size]));
            if res != SQUASH_PROCESSING {
                break;
            }
        }
        Ok(())
    }
}

impl Drop for CompressionStream {
    fn drop(&mut self) {
        unsafe {
            //squash_object_unref(self.stream as *mut ::std::os::raw::c_void);
            use libc;
            squash_object_unref(self.stream as *mut libc::c_void);
        }
    }
}


mod tests {

    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_parse() {
        let method = Compression::from_string("deflate/1").unwrap();
        assert_eq!(("deflate", 1), (method.name(), method.level()));
        let method = Compression::from_string("zlib/2").unwrap();
        assert_eq!(("deflate", 2), (method.name(), method.level()));
        let method = Compression::from_string("gzip/3").unwrap();
        assert_eq!(("deflate", 3), (method.name(), method.level()));
        let method = Compression::from_string("brotli/1").unwrap();
        assert_eq!(("brotli", 1), (method.name(), method.level()));
        let method = Compression::from_string("lzma/1").unwrap();
        assert_eq!(("lzma", 1), (method.name(), method.level()));
        let method = Compression::from_string("lzma2/2").unwrap();
        assert_eq!(("lzma", 2), (method.name(), method.level()));
        let method = Compression::from_string("xz/3").unwrap();
        assert_eq!(("lzma", 3), (method.name(), method.level()));
        let method = Compression::from_string("lz4/1").unwrap();
        assert_eq!(("lz4", 1), (method.name(), method.level()));
    }

    #[test]
    fn test_to_string() {
        assert_eq!(
            "brotli/1",
            Compression::from_string("brotli/1").unwrap().to_string()
        );
        assert_eq!(
            "deflate/1",
            Compression::from_string("gzip/1").unwrap().to_string()
        );
    }

    #[allow(dead_code, needless_range_loop)]
    fn test_data(n: usize) -> Vec<u8> {
        let mut input = vec![0; n];
        for i in 0..input.len() {
            input[i] = (i * i * i) as u8;
        }
        input
    }

    #[allow(dead_code)]
    fn test_compression(method: &str, min_lvl: u8, max_lvl: u8) {
        let input = test_data(16 * 1024);
        for i in min_lvl..max_lvl + 1 {
            let method = Compression::from_string(&format!("{}/{}", method, i)).unwrap();
            println!("{}", method.to_string());
            let compressed = method.compress(&input).unwrap();
            let decompressed = method.decompress(&compressed).unwrap();
            assert_eq!(input.len(), decompressed.len());
            for i in 0..input.len() {
                assert_eq!(input[i], decompressed[i]);
            }
        }
    }

    #[test]
    fn test_compression_deflate() {
        test_compression("deflate", 1, 9)
    }

    #[test]
    fn test_compression_brotli() {
        test_compression("brotli", 1, 11)
    }

    #[test]
    fn test_compression_lzma() {
        test_compression("lzma", 1, 9)
    }

    #[test]
    fn test_compression_lz4() {
        test_compression("lz4", 1, 11)
    }

    #[allow(dead_code)]
    fn test_stream_compression(method: &str, min_lvl: u8, max_lvl: u8) {
        let input = test_data(512 * 1024);
        for i in min_lvl..max_lvl + 1 {
            let method = Compression::from_string(&format!("{}/{}", method, i)).unwrap();
            println!("{}", method.to_string());
            let mut compressor = method.compress_stream().unwrap();
            let mut compressed = Vec::with_capacity(input.len());
            compressor.process(&input, &mut compressed).unwrap();
            compressor.finish(&mut compressed).unwrap();
            let mut decompressor = method.decompress_stream().unwrap();
            let mut decompressed = Vec::with_capacity(input.len());
            decompressor
                .process(&compressed, &mut decompressed)
                .unwrap();
            decompressor.finish(&mut decompressed).unwrap();
            assert_eq!(input.len(), decompressed.len());
            for i in 0..input.len() {
                assert_eq!(input[i], decompressed[i]);
            }
        }
    }

    #[test]
    fn test_stream_compression_deflate() {
        test_stream_compression("deflate", 1, 9)
    }

    #[test]
    fn test_stream_compression_brotli() {
        test_stream_compression("brotli", 1, 11)
    }

    #[test]
    fn test_stream_compression_lzma() {
        test_stream_compression("lzma", 1, 9)
    }

    #[test]
    fn test_stream_compression_lz4() {
        test_stream_compression("lz4", 1, 11)
    }

}


#[cfg(feature = "bench")]
mod benches {

    #[allow(unused_imports)]
    use super::*;

    use test::Bencher;


    #[allow(dead_code, needless_range_loop)]
    fn test_data(n: usize) -> Vec<u8> {
        let mut input = vec![0; n];
        for i in 0..input.len() {
            input[i] = (i * i * i) as u8;
        }
        input
    }

    #[allow(dead_code)]
    fn bench_stream_compression(b: &mut Bencher, method: Compression) {
        let input = test_data(512 * 1024);
        b.iter(|| {
            let mut compressor = method.compress_stream().unwrap();
            let mut compressed = Vec::with_capacity(input.len());
            compressor.process(&input, &mut compressed).unwrap();
            compressor.finish(&mut compressed).unwrap();
        });
        b.bytes = input.len() as u64;
    }

    #[allow(dead_code)]
    fn bench_stream_decompression(b: &mut Bencher, method: Compression) {
        let input = test_data(512 * 1024);
        let mut compressor = method.compress_stream().unwrap();
        let mut compressed = Vec::with_capacity(input.len());
        compressor.process(&input, &mut compressed).unwrap();
        compressor.finish(&mut compressed).unwrap();
        b.iter(|| {
            let mut decompressor = method.decompress_stream().unwrap();
            let mut decompressed = Vec::with_capacity(compressed.len());
            decompressor
                .process(&compressed, &mut decompressed)
                .unwrap();
            decompressor.finish(&mut decompressed).unwrap();
        });
        b.bytes = input.len() as u64;
    }

    #[bench]
    fn bench_deflate_1_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("deflate/1").unwrap())
    }

    #[bench]
    fn bench_deflate_2_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("deflate/2").unwrap())
    }

    #[bench]
    fn bench_deflate_3_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("deflate/3").unwrap())
    }

    #[bench]
    fn bench_deflate_4_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("deflate/4").unwrap())
    }

    #[bench]
    fn bench_deflate_5_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("deflate/5").unwrap())
    }

    #[bench]
    fn bench_deflate_6_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("deflate/6").unwrap())
    }

    #[bench]
    fn bench_deflate_7_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("deflate/7").unwrap())
    }

    #[bench]
    fn bench_deflate_8_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("deflate/8").unwrap())
    }

    #[bench]
    fn bench_deflate_9_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("deflate/9").unwrap())
    }

    #[bench]
    fn bench_deflate_1_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("deflate/1").unwrap())
    }

    #[bench]
    fn bench_deflate_2_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("deflate/2").unwrap())
    }

    #[bench]
    fn bench_deflate_3_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("deflate/3").unwrap())
    }

    #[bench]
    fn bench_deflate_4_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("deflate/4").unwrap())
    }

    #[bench]
    fn bench_deflate_5_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("deflate/5").unwrap())
    }

    #[bench]
    fn bench_deflate_6_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("deflate/6").unwrap())
    }

    #[bench]
    fn bench_deflate_7_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("deflate/7").unwrap())
    }

    #[bench]
    fn bench_deflate_8_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("deflate/8").unwrap())
    }

    #[bench]
    fn bench_deflate_9_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("deflate/9").unwrap())
    }


    #[bench]
    fn bench_brotli_1_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("brotli/1").unwrap())
    }

    #[bench]
    fn bench_brotli_2_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("brotli/2").unwrap())
    }

    #[bench]
    fn bench_brotli_3_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("brotli/3").unwrap())
    }

    #[bench]
    fn bench_brotli_4_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("brotli/4").unwrap())
    }

    #[bench]
    fn bench_brotli_5_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("brotli/5").unwrap())
    }

    #[bench]
    fn bench_brotli_6_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("brotli/6").unwrap())
    }

    #[bench]
    fn bench_brotli_7_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("brotli/7").unwrap())
    }

    #[bench]
    fn bench_brotli_8_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("brotli/8").unwrap())
    }

    #[bench]
    fn bench_brotli_9_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("brotli/9").unwrap())
    }

    #[bench]
    fn bench_brotli_10_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("brotli/10").unwrap())
    }

    #[bench]
    fn bench_brotli_11_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("brotli/11").unwrap())
    }

    #[bench]
    fn bench_brotli_1_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("brotli/1").unwrap())
    }

    #[bench]
    fn bench_brotli_2_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("brotli/2").unwrap())
    }

    #[bench]
    fn bench_brotli_3_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("brotli/3").unwrap())
    }

    #[bench]
    fn bench_brotli_4_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("brotli/4").unwrap())
    }

    #[bench]
    fn bench_brotli_5_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("brotli/5").unwrap())
    }

    #[bench]
    fn bench_brotli_6_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("brotli/6").unwrap())
    }

    #[bench]
    fn bench_brotli_7_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("brotli/7").unwrap())
    }

    #[bench]
    fn bench_brotli_8_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("brotli/8").unwrap())
    }

    #[bench]
    fn bench_brotli_9_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("brotli/9").unwrap())
    }

    #[bench]
    fn bench_brotli_10_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("brotli/10").unwrap())
    }

    #[bench]
    fn bench_brotli_11_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("brotli/11").unwrap())
    }


    #[bench]
    fn bench_lzma_1_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lzma/1").unwrap())
    }

    #[bench]
    fn bench_lzma_2_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lzma/2").unwrap())
    }

    #[bench]
    fn bench_lzma_3_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lzma/3").unwrap())
    }

    #[bench]
    fn bench_lzma_4_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lzma/4").unwrap())
    }

    #[bench]
    fn bench_lzma_5_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lzma/5").unwrap())
    }

    #[bench]
    fn bench_lzma_6_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lzma/6").unwrap())
    }

    #[bench]
    fn bench_lzma_7_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lzma/7").unwrap())
    }

    #[bench]
    fn bench_lzma_8_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lzma/8").unwrap())
    }

    #[bench]
    fn bench_lzma_9_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lzma/9").unwrap())
    }

    #[bench]
    fn bench_lzma_1_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lzma/1").unwrap())
    }

    #[bench]
    fn bench_lzma_2_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lzma/2").unwrap())
    }

    #[bench]
    fn bench_lzma_3_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lzma/3").unwrap())
    }

    #[bench]
    fn bench_lzma_4_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lzma/4").unwrap())
    }

    #[bench]
    fn bench_lzma_5_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lzma/5").unwrap())
    }

    #[bench]
    fn bench_lzma_6_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lzma/6").unwrap())
    }

    #[bench]
    fn bench_lzma_7_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lzma/7").unwrap())
    }

    #[bench]
    fn bench_lzma_8_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lzma/8").unwrap())
    }

    #[bench]
    fn bench_lzma_9_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lzma/9").unwrap())
    }


    #[bench]
    fn bench_lz4_1_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lz4/1").unwrap())
    }

    #[bench]
    fn bench_lz4_2_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lz4/2").unwrap())
    }

    #[bench]
    fn bench_lz4_3_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lz4/3").unwrap())
    }

    #[bench]
    fn bench_lz4_4_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lz4/4").unwrap())
    }

    #[bench]
    fn bench_lz4_5_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lz4/5").unwrap())
    }

    #[bench]
    fn bench_lz4_6_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lz4/6").unwrap())
    }

    #[bench]
    fn bench_lz4_7_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lz4/7").unwrap())
    }

    #[bench]
    fn bench_lz4_8_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lz4/8").unwrap())
    }

    #[bench]
    fn bench_lz4_9_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lz4/9").unwrap())
    }

    #[bench]
    fn bench_lz4_10_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lz4/10").unwrap())
    }

    #[bench]
    fn bench_lz4_11_compress(b: &mut Bencher) {
        bench_stream_compression(b, Compression::from_string("lz4/11").unwrap())
    }

    #[bench]
    fn bench_lz4_1_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lz4/1").unwrap())
    }

    #[bench]
    fn bench_lz4_2_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lz4/2").unwrap())
    }

    #[bench]
    fn bench_lz4_3_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lz4/3").unwrap())
    }

    #[bench]
    fn bench_lz4_4_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lz4/4").unwrap())
    }

    #[bench]
    fn bench_lz4_5_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lz4/5").unwrap())
    }

    #[bench]
    fn bench_lz4_6_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lz4/6").unwrap())
    }

    #[bench]
    fn bench_lz4_7_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lz4/7").unwrap())
    }

    #[bench]
    fn bench_lz4_8_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lz4/8").unwrap())
    }

    #[bench]
    fn bench_lz4_9_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lz4/9").unwrap())
    }

    #[bench]
    fn bench_lz4_10_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lz4/10").unwrap())
    }

    #[bench]
    fn bench_lz4_11_decompress(b: &mut Bencher) {
        bench_stream_decompression(b, Compression::from_string("lz4/11").unwrap())
    }

}
