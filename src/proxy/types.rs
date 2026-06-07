use tokio_util::bytes::Bytes;

#[derive(PartialEq, Eq, Hash)]
pub struct ServerName(Bytes);

impl ServerName {
    pub fn new(server_name: String) -> Self {
        Self(Bytes::from(server_name))
    }
}

impl From<Bytes> for ServerName {
    fn from(value: Bytes) -> Self {
        Self(value)
    }
}
