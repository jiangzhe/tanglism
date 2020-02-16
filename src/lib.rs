pub mod morph;
pub mod reshape;
pub mod data;

pub type TTimestamp = i64;
pub type TPrice = f64;
pub type TQuantity = u64;


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
