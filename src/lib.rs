mod glob;
mod grab;
mod grab_flags;
mod helpers;
mod prep_article;
mod readability;
mod score;

pub use readability::Article;
pub use readability::Config;
pub use readability::Metadata;
pub use readability::Readability;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReadabilityError {
    #[error(transparent)]
    BadDocumentURL(#[from] url::ParseError),
    #[error("failed to grab the article")]
    GrabFailed,
    #[error("too many elements ({0} > {1}) in the documents to parse")]
    TooManyElements(usize, usize),
}
