pub trait Singleton {
    fn instance() -> &'static Self;
}
