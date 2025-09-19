use actix_web::middleware::Compress;

pub struct Compression;

impl Default for Compression {
    fn default() -> Compress {
        Compress::default()
    }
}