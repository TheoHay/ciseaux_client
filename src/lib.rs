pub mod clients;

pub const DEFAULT_CONN_PER_THREAD: usize = 4;

pub enum CiseauxError {
    NetworkError,
    CommandError,
}
