use dom_query::Matcher;
use once_cell::sync::Lazy;
use phf::phf_set;
use regex::Regex;

macro_rules! lazy_re {
    ($pattern:expr) => {
        Lazy::new(|| Regex::new($pattern).unwrap())
    };
}

macro_rules! lazy_matcher {
    ($pattern:expr) => {
        Lazy::new(|| Matcher::new($pattern).unwrap())
    };
}
pub(crate) static CONTENT_ID: &str = "readability-page-1";
pub(crate) static MIN_COMMON_ANCESTORS: usize = 3;
pub(crate) static SCORE_ATTR: &str = "data-readability-score";
pub(crate) static MIN_SCORE: f32 = 20.0;
pub(crate) static MIN_CONTENT_LENGTH: usize = 140;

pub(crate) static MATCHER_CONTENT_ID: Lazy<Matcher> = lazy_matcher!(&format!("#{}", CONTENT_ID));
pub(crate) static MATCHER_LI_P: Lazy<Matcher> = lazy_matcher!("li p");
pub(crate) static MATCHER_TITLE: Lazy<Matcher> = lazy_matcher!("head title");
pub(crate) static MATCHER_SCRIPT: Lazy<Matcher> = lazy_matcher!("script, noscript");
pub(crate) static MATCHER_HTML_LANG: Lazy<Matcher> = lazy_matcher!("html[lang]");
pub(crate) static MATCHER_STYLE: Lazy<Matcher> = lazy_matcher!("style");
pub(crate) static MATCHER_FONT: Lazy<Matcher> = lazy_matcher!("font");
pub(crate) static MATCHER_BR: Lazy<Matcher> = lazy_matcher!("br");
pub(crate) static MATCHER_IMG: Lazy<Matcher> = lazy_matcher!("img");
pub(crate) static MATCHER_META: Lazy<Matcher> = lazy_matcher!("meta[content]");
pub(crate) static MATCHER_JS_LINK: Lazy<Matcher> = lazy_matcher!(r#"a[href^="javascript:"]"#);
pub(crate) static MATCHER_JSONLD: Lazy<Matcher> =
    lazy_matcher!(r#"script[type="application/ld+json"]"#);
pub(crate) static MATCHER_HEADING: Lazy<Matcher> = lazy_matcher!(r#"h1,h2"#);
pub(crate) static MATCHER_DIALOGS: Lazy<Matcher> =
    lazy_matcher!(r#"*[aria-modal="true"][role="dialog"]"#);
pub(crate) static MATCHER_BYLINE: Lazy<Matcher> =
    lazy_matcher!(r#"[rel="author"],[itemprop*="author"]"#);
pub(crate) static MATCHER_SOURCES: Lazy<Matcher> =
    lazy_matcher!("img,picture,figure,video,audio,sources");
pub(crate) static MATCHER_P: Lazy<Matcher> = lazy_matcher!("p");
pub(crate) static MATCHER_EMBEDS: Lazy<Matcher> = lazy_matcher!("object,embed,iframe");

pub(crate) static META_TITLE_KEYS: &[&str] = &[
    "dc:title",
    "dcterm:title",
    "og:title",
    "weibo:article:title",
    "weibo:webpage:title",
    "title",
    "twitter:title",
    "parsely-title",
];
pub(crate) static META_IMAGE_KEYS: &[&str] = &["og:image", "image", "twitter:image"];
pub(crate) static META_MOD_TIME_KEYS: &[&str] = &["article:modified_time", "dcterms.modifie"];
pub(crate) static META_PUB_TIME_KEYS: &[&str] = &[
    "article:published_time",
    "dcterms.available",
    "dcterms.created",
    "dcterms.issued",
    "parsely-pub-date",
    "weibo:article:create_at",
];
pub(crate) static META_BYLINE_KEYS: &[&str] =
    &["dc:creator", "dcterms:creator", "author", "parsely-author"];
pub(crate) static META_EXCERPT_KEYS: &[&str] = &[
    "dc:description",
    "dcterm:description",
    "og:description",
    "weibo:article:description",
    "weibo:webpage:description",
    "description",
    "twitter:description",
];

pub(crate) static TEXTISH_TAGS: &str = "blockquote,dl,div,img,ol,p,pre,table,ul,span,li,td";

pub(crate) static PRESENTATIONAL_ATTRIBUTES: &[&str] = &[
    "align",
    "background",
    "bgcolor",
    "border",
    "cellpadding",
    "cellspacing",
    "frame",
    "hspace",
    "rules",
    "style",
    "valign",
    "vspace",
];

pub(crate) static UNLIKELY_CANDIDATES: &[&str] = &[
    "-ad-",
    "ai2html",
    "banner",
    "breadcrumbs",
    "combx",
    "comment",
    "community",
    "cover-wrap",
    "disqus",
    "extra",
    "footer",
    "gdpr",
    "header",
    "legends",
    "menu",
    "related",
    "remark",
    "replies",
    "rss",
    "shoutbox",
    "sidebar",
    "skyscraper",
    "social",
    "sponsor",
    "supplemental",
    "ad-break",
    "agegate",
    "pagination",
    "pager",
    "popup",
    "yom-remote",
];

pub(crate) static MAYBE_CANDIDATES: &[&str] = &[
    "and", "article", "body", "column", "content", "main", "shadow", "layout",
];

pub(crate) static BYLINE_PATTERNS: &[&str] =
    &["byline", "author", "dateline", "writtenby", "p-author"];

pub(crate) static JSONLD_ARTICLE_TYPES: &[&str] = &[
    "Article",
    "AdvertiserContentArticle",
    "NewsArticle",
    "AnalysisNewsArticle",
    "AskPublicNewsArticle",
    "BackgroundNewsArticle",
    "OpinionNewsArticle",
    "ReportageNewsArticle",
    "ReviewNewsArticle",
    "Report",
    "SatiricalArticle",
    "ScholarlyArticle",
    "MedicalScholarlyArticle",
    "SocialMediaPosting",
    "BlogPosting",
    "LiveBlogPosting",
    "DiscussionForumPosting",
    "TechArticle",
    "APIReference",
];

pub(crate) static COMMAS: &[char] = &[
    '\u{002C}', '\u{060C}', '\u{FE50}', '\u{FE10}', '\u{FE11}', '\u{2E41}', '\u{2E34}', '\u{2E32}',
    '\u{FF0C}',
];

pub(crate) static TITLE_SEPARATORS: &[char] = &['|', '-', '\\', '/', '>', '»'];
pub(crate) static TITLE_HIERARCHY_SEP: &[char] = &['\\', '/', '>', '»'];

#[rustfmt::skip]
pub(crate) static BLOCK_ELEMS: phf::Set<&'static str> = phf_set!(
    "blockquote", "dl", "div", "img", "ol", "p", "pre", "table", "ul",
);

pub(crate) static ALTER_TO_DIV_EXCEPTIONS: phf::Set<&'static str> =
    phf_set!("article", "section", "p", "ol", "ul");

pub(crate) static DEFAULT_TAGS_TO_SCORE: phf::Set<&'static str> =
    phf_set!("section", "h2", "h3", "h4", "h5", "h6", "p", "td", "pre");

pub(crate) static TAGS_WITH_CONTENT: phf::Set<&'static str> =
    phf_set!("div", "section", "header", "h1", "h2", "h3", "h4", "h5", "h6");

pub(crate) static EMBED_ELEMENTS: phf::Set<&'static str> = phf_set!("object", "embed", "iframe");

#[rustfmt::skip]
pub(crate) static UNLIKELY_ROLES: phf::Set<&'static str> = phf_set!(
    "menu", "menubar", "complementary", "navigation", "alert", "alertdialog", "dialog"
);

#[rustfmt::skip]
pub(crate) static PHRASING_ELEMS: phf::Set<&'static str> = phf_set!(
    "abbr", "audio", "b", "bdo", "br", "button", "cite", "code", "data", "datalist", "dfn", "em",
    "embed", "i", "img", "input", "kbd", "label", "mark", "math", "meter", "noscript", "object",
    "output", "progress", "q", "ruby", "samp", "script", "select", "small", "span", "strong",
    "sub", "sup", "textarea", "time", "var", "wbr"
);

#[rustfmt::skip]
pub(crate) static CLASSES_NEGATIVE: phf::Set<&'static str> = phf_set!(
    "-ad-", "hidden", "banner", "combx", "comment", "com-", "contact", "footer",
    "gdpr", "masthead", "media", "meta", "outbrain", "promo", "related", "scroll",
    "share", "shoutbox", "sidebar", "skyscraper", "sponsor", "shopping", "tags",
    "widget"
);

#[rustfmt::skip]
pub(crate) static CLASSES_POSITIVE: phf::Set<&'static str> = phf_set!(
    "article", "body", "content", "entry", "hentry", "h-entry", "main", "page",
    "pagination", "post", "text", "blog", "story",
);

pub(crate) static DEPRECATED_SIZE_ATTRIBUTE_ELEMS: phf::Set<&'static str> =
    phf_set!("table", "th", "td", "hr", "pre");

#[rustfmt::skip]
pub(crate) static AD_WORDS: phf::Set<&'static str> = phf_set!(
    "ad", "advertising", "advertisement", "pub", "publicité", 
    "werb", "werbung", "广告", "реклама", "anuncio"
);
#[rustfmt::skip]
pub(crate) static LOADING_WORDS: phf::Set<&'static str> = phf_set!(
    "loading", "正在加载", "загрузка", "chargement", "cargando"
);

pub(crate) static SHARE_WORDS: phf::Set<&'static str> = phf_set!("share", "sharedaddy");

pub(crate) static RX_STYLE_DISPLAY_NONE: Lazy<Regex> =
    lazy_re!(r#"display\s*:\s*none|visibility\s*:\s*hidden"#);
pub(crate) static RX_CDATA: Lazy<Regex> = lazy_re!(r#"^\s*<!\[CDATA\[|\]\]>\s*$"#);
pub(crate) static RX_SCHEMA_ORG: Lazy<Regex> = lazy_re!(r#"^https?://schema\.org/?$"#);

pub(crate) static RX_TITLE_W_LAST: Lazy<Regex> = lazy_re!(r#"(.*)[\|\-\\/>»] .*"#);
pub(crate) static RX_TITLE_W_FIRST: Lazy<Regex> = lazy_re!(r#"[^\|\-\\/>»]*[\|\-\\/>»](.*)"#);

pub(crate) static RX_META_NAME: Lazy<Regex> = lazy_re!(
    r#"(?:(dc|dcterm|og|twitter|parsely|weibo:(article|webpage))\s*[-\.:]\s*)?(author|creator|pub-date|description|title|site_name)"#
);
pub(crate) static RX_META_PROPERTY: Lazy<Regex> = lazy_re!(
    r#"\s*(article|dc|dcterm|og|twitter)\s*:\s*(author|creator|description|published_time|title|site_name)\s*"#
);

pub(crate) static RX_CLASSES_NEGATIVE: Lazy<Regex> = lazy_re!(r"\bhid\b");

pub(crate) static RX_VIDEO_ATTRS: Lazy<Regex> = lazy_re!(
    r#"//(www\.)?((dailymotion|youtube|youtube-nocookie|player\.vimeo|v\.qq)\.com|(archive|upload\.wikimedia)\.org|player\.twitch\.tv)"#
);
pub(crate) static RX_BASE64_URL: Lazy<Regex> = lazy_re!(r#"^data:\s*([^\s;,]+)\s*;\s*base64\s*"#);
pub(crate) static RX_IMG_ATTR: Lazy<Regex> = lazy_re!(r#".(jpg|jpeg|png|webp)"#);
pub(crate) static RX_IMG_ATTR_TO_SRC: Lazy<Regex> =
    lazy_re!(r#"^\s*\S+\.(jpg|jpeg|png|webp)\S*\s*$"#);
pub(crate) static RX_IMG_ATTR_TO_SRCSET: Lazy<Regex> = lazy_re!(r#".(jpg|jpeg|png|webp)\s+\d"#);
