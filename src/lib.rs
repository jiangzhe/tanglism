pub mod morph;

pub type TTimestamp = u64;
pub type TPrice = f64;
pub type TQuantity = u64;


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
