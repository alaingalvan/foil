pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn err<'a, T>(msg: &'a str) -> Result<T> {
    Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        msg,
    )))
}

#[macro_export]
macro_rules! return_err {
    // `()` indicates that the macro takes no argument.
    ($a:expr,$b:expr) => {
        match $a {
            Ok(v) => v,
            Err(e) => {
                println!("{:?}", e);
                return crate::error::err($b);
            }
        }
    };
}
