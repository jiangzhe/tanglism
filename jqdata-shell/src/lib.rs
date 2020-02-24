mod error;
mod insert;
mod select;
mod datetime;
pub use error::Error;
pub use insert::*;
pub use select::*;
pub type Result<T> = std::result::Result<T, Error>;

// autocomplete stock code,
// support 600, 601, 603, 000, 002, 300
fn code_autocomplete(code: &str) -> Result<String> {
    if code.len() != 6 && code.len() != 11 {
        return Err(Error(format!("invalid code: {}", code)));
    }
    if code.len() == 11 {
        if code.ends_with(".XSHG") || code.ends_with(".XSHE") {
            return Ok(code.to_owned());
        }
        return Err(Error(format!("invalid code: {}", code)));
    }
    let result = match &code[0..3] {
        "600" => format!("{}.XSHG", code),
        "601" => format!("{}.XSHG", code),
        "603" => format!("{}.XSHG", code),
        "000" => format!("{}.XSHE", code),
        "002" => format!("{}.XSHE", code),
        "300" => format!("{}.XSHE", code),
        _ => return Err(Error(format!("invalid code: {}", code))),
    };
    Ok(result)
}

// normalize datetime to format for request
fn request_datetime(dt: &str) -> Result<String> {
    match dt.len() {
        10 => Ok(format!("{} 00:00:00", dt)),
        13 => Ok(format!("{}:00:00", dt)),
        16 => Ok(format!("{}:00", dt)),
        19 => Ok(dt.to_owned()),
        _ => Err(Error(format!("invalid datetime format: {}", dt))),
    }
}