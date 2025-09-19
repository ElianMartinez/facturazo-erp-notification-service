use actix_web::middleware::Compress;

pub type Compression = Compress;

pub fn create_compression_middleware() -> Compression {
    Compress::default()
}