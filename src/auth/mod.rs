pub mod github;
pub mod session;
pub mod supabase;

pub use session::Session;

/// Messages sent from the background device-flow polling thread back to the UI thread.
#[derive(Debug)]
pub enum AuthUpdate {
    Completed(Session),
    Failed(String),
}
