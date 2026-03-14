pub trait OkAble: Sized {
    fn ok<Error>(self) -> Result<Self, Error>;
}

impl<T> OkAble for T {
    fn ok<Error>(self) -> Result<Self, Error> {
        Ok(self)
    }
}
