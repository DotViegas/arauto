//! What the panel needs of HTTP and the hub had no use for until now: reading a cookie and a
//! small form, and answering a page, a redirect or a file with the headers that keep a
//! browser from doing more than showing it.

use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Response, StatusCode, header};
use http_body_util::{BodyExt, Limited};
use hyper::body::Incoming;
use zeroize::Zeroizing;

/// The largest form the panel reads. The entry code is 39 characters; what a browser answers
/// in a passkey ceremony is a few thousand, written as a form field.
const FORM_MAX: usize = 16 * 1024;

/// How long the body of a form may take to arrive.
const FORM_READ: Duration = Duration::from_secs(5);

/// What a page of the panel may load: its own sheet, its own fonts, its own script file, and
/// nothing else. No script written inside a page runs, nothing is framed, a form only posts
/// back to the panel, and the script only asks the panel itself (for the challenge of a
/// passkey ceremony).
const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src 'self'; style-src 'self'; \
     font-src 'self'; img-src 'self'; connect-src 'self'; form-action 'self'; \
     frame-ancestors 'none'; base-uri 'none'";

/// The headers every answer of the panel carries.
fn guarded(mut response: Response<Body>, cache: &'static str) -> Response<Body> {
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CONTENT_SECURITY_POLICY),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

/// A page. It is never stored by the browser nor by anything on the way.
pub(super) fn page(status: StatusCode, html: String) -> Response<Body> {
    let mut response = Response::new(Body::from(html));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    guarded(response, "no-store")
}

/// A status and one fixed line, for what is not a page: a wrong address, a wrong method, a
/// request that did not come from the panel.
pub(super) fn plain(status: StatusCode, line: &'static str) -> Response<Body> {
    let mut response = Response::new(Body::from(line));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    guarded(response, "no-store")
}

/// What the script of a page asked for: the options of a passkey ceremony, as JSON written
/// by the library of the ceremonies. Never stored.
pub(super) fn json(body: String) -> Response<Body> {
    let mut response = Response::new(Body::from(body));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    guarded(response, "no-store")
}

/// "See the other address": after a form, and when a page needs a session there is none of.
pub(super) fn see_other(to: &'static str) -> Response<Body> {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::SEE_OTHER;
    response
        .headers_mut()
        .insert(header::LOCATION, HeaderValue::from_static(to));
    guarded(response, "no-store")
}

/// "See the other address", for an address made of a name the hub already checked. One that
/// is not a header value leads to the first page.
pub(super) fn see_other_to(to: &str) -> Response<Body> {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::SEE_OTHER;
    let location = HeaderValue::from_str(to).unwrap_or_else(|_| HeaderValue::from_static("/"));
    response.headers_mut().insert(header::LOCATION, location);
    guarded(response, "no-store")
}

/// A file of the panel itself: the sheet, the fonts and the script.
pub(super) fn file(content_type: &'static str, bytes: &'static [u8]) -> Response<Body> {
    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    guarded(response, "private, max-age=300")
}

/// Adds a `Set-Cookie` to an answer. A value that is not a header value sets nothing.
pub(super) fn with_cookie(mut response: Response<Body>, cookie: &str) -> Response<Body> {
    if let Ok(mut value) = HeaderValue::from_str(cookie) {
        // The identifier of a session is not for whatever prints headers.
        value.set_sensitive(true);
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}

/// The value of the cookie `name`, when the request brings exactly that cookie.
pub(super) fn cookie<'h>(headers: &'h HeaderMap, name: &str) -> Option<&'h str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|line| line.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(found, _)| *found == name)
        .map(|(_, value)| value)
}

/// Whether the one value of a header is exactly `expected`. A header that is missing,
/// repeated or not text is not.
pub(super) fn header_is(headers: &HeaderMap, name: header::HeaderName, expected: &str) -> bool {
    let mut values = headers.get_all(name).iter();
    match (values.next(), values.next()) {
        (Some(value), None) => value
            .to_str()
            .is_ok_and(|text| text.eq_ignore_ascii_case(expected)),
        _ => false,
    }
}

/// Why a form was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FormError {
    /// It is not `application/x-www-form-urlencoded`.
    NotAForm,
    /// It is larger than a form of the panel is.
    TooLarge,
    /// It took too long, or the connection failed.
    NotRead,
}

impl FormError {
    pub(super) const fn status(self) -> StatusCode {
        match self {
            FormError::NotAForm => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            FormError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            FormError::NotRead => StatusCode::BAD_REQUEST,
        }
    }
}

/// A form sent by a page of the panel, as it came. What was typed in it is erased from
/// memory when it is dropped.
pub(super) struct Form(Zeroizing<Vec<u8>>);

impl Form {
    /// The value of a field, decoded. A field that did not come, or is not text, is none.
    pub(super) fn field(&self, name: &str) -> Option<Zeroizing<String>> {
        field_of(&self.0, name)
    }
}

/// Reads one field of a form sent by a page of the panel. What was typed is erased from
/// memory when the caller drops it.
pub(super) async fn form_field(
    headers: &HeaderMap,
    body: Incoming,
    field: &str,
) -> Result<Option<Zeroizing<String>>, FormError> {
    Ok(form(headers, body).await?.field(field))
}

/// The value of `field` in the query of an address.
pub(super) fn query_field(query: Option<&str>, field: &str) -> Option<Zeroizing<String>> {
    field_of(query?.as_bytes(), field)
}

/// Reads a form sent by a page of the panel: small, and of the one type a form of a page has.
pub(super) async fn form(headers: &HeaderMap, body: Incoming) -> Result<Form, FormError> {
    let essence = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim());
    if !essence
        .is_some_and(|essence| essence.eq_ignore_ascii_case("application/x-www-form-urlencoded"))
    {
        return Err(FormError::NotAForm);
    }
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok());
    if declared.is_some_and(|declared| declared > FORM_MAX) {
        return Err(FormError::TooLarge);
    }
    let read = tokio::time::timeout(FORM_READ, Limited::new(body, FORM_MAX).collect()).await;
    let bytes = match read {
        Ok(Ok(collected)) => Zeroizing::new(collected.to_bytes().to_vec()),
        Ok(Err(error)) if error.is::<http_body_util::LengthLimitError>() => {
            return Err(FormError::TooLarge);
        }
        Ok(Err(_)) | Err(_) => return Err(FormError::NotRead),
    };
    Ok(Form(bytes))
}

/// The value of `field` in a form body, decoded. A value that is not text is none.
fn field_of(body: &[u8], field: &str) -> Option<Zeroizing<String>> {
    body.split(|byte| *byte == b'&').find_map(|pair| {
        let mut halves = pair.splitn(2, |byte| *byte == b'=');
        let name = halves.next()?;
        let value = halves.next().unwrap_or_default();
        if decode(name)?.as_str() != field {
            return None;
        }
        decode(value)
    })
}

/// Decodes the encoding of a form: `+` is a space and `%XX` is a byte.
fn decode(encoded: &[u8]) -> Option<Zeroizing<String>> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(encoded.len()));
    let mut rest = encoded.iter().copied();
    while let Some(byte) = rest.next() {
        match byte {
            b'+' => bytes.push(b' '),
            b'%' => {
                let high = char::from(rest.next()?).to_digit(16)?;
                let low = char::from(rest.next()?).to_digit(16)?;
                bytes.push(u8::try_from(high.checked_mul(16)?.checked_add(low)?).ok()?);
            }
            other => bytes.push(other),
        }
    }
    std::str::from_utf8(&bytes)
        .ok()
        .map(|text| Zeroizing::new(text.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(*name, HeaderValue::from_static(value));
        }
        map
    }

    #[test]
    fn a_cookie_is_found_only_by_its_exact_name() {
        let found = headers(&[("cookie", "theme=dark; arauto-panel=abc123; other=1")]);
        assert_eq!(cookie(&found, "arauto-panel"), Some("abc123"));
        assert_eq!(cookie(&found, "panel"), None);
        assert_eq!(cookie(&found, "__Host-arauto-panel"), None);
        let two = headers(&[("cookie", "a=1"), ("cookie", "arauto-panel=xyz")]);
        assert_eq!(cookie(&two, "arauto-panel"), Some("xyz"));
        assert_eq!(cookie(&headers(&[]), "arauto-panel"), None);
    }

    #[test]
    fn a_header_is_what_is_expected_only_when_it_comes_once_and_equal() {
        let one = headers(&[("origin", "https://panel.example.com")]);
        assert!(header_is(&one, header::ORIGIN, "https://panel.example.com"));
        assert!(!header_is(
            &one,
            header::ORIGIN,
            "https://panel.example.com.evil.test"
        ));
        assert!(!header_is(&one, header::ORIGIN, "https://panel.example.co"));
        let twice = headers(&[
            ("origin", "https://panel.example.com"),
            ("origin", "https://evil.test"),
        ]);
        assert!(!header_is(
            &twice,
            header::ORIGIN,
            "https://panel.example.com"
        ));
        assert!(!header_is(
            &headers(&[]),
            header::ORIGIN,
            "https://panel.example.com"
        ));
        assert!(!header_is(
            &headers(&[("origin", "null")]),
            header::ORIGIN,
            "https://panel.example.com"
        ));
    }

    #[test]
    fn a_form_field_is_decoded_and_the_others_are_ignored() {
        let body = b"other=1&code=abcd-efgh+ijkl%2Dmnop&code=second";
        assert_eq!(
            field_of(body, "code").unwrap().as_str(),
            "abcd-efgh ijkl-mnop"
        );
        assert_eq!(field_of(body, "other").unwrap().as_str(), "1");
        assert!(field_of(body, "missing").is_none());
        assert_eq!(field_of(b"code", "code").unwrap().as_str(), "");
        // A broken escape, or bytes that are not text, are no value at all.
        assert!(field_of(b"code=%4", "code").is_none());
        assert!(field_of(b"code=%zz", "code").is_none());
        assert!(field_of(b"code=%ff%fe", "code").is_none());
    }

    #[test]
    fn every_answer_tells_the_browser_to_do_no_more_than_show_it() {
        for response in [
            page(StatusCode::OK, "<p>x</p>".to_owned()),
            plain(StatusCode::NOT_FOUND, "not found\n"),
            see_other("/sign-in"),
            file("text/css; charset=utf-8", b"body{}"),
        ] {
            let headers = response.headers();
            let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
            assert!(policy.contains("default-src 'none'"));
            assert!(policy.contains("frame-ancestors 'none'"));
            assert!(policy.contains("form-action 'self'"));
            assert!(!policy.contains("unsafe"), "{policy}");
            // The one script that runs is a file of the panel: none written in a page.
            assert!(policy.contains("script-src 'self';"), "{policy}");
            assert_eq!(policy.matches("script-src").count(), 1, "{policy}");
            assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
            assert_eq!(headers[header::X_FRAME_OPTIONS], "DENY");
            assert_eq!(headers[header::REFERRER_POLICY], "no-referrer");
            assert!(headers.contains_key(header::CACHE_CONTROL));
        }
        // A page, a refusal and a redirect are never stored.
        assert_eq!(
            page(StatusCode::OK, String::new()).headers()[header::CACHE_CONTROL],
            "no-store"
        );
        assert_eq!(see_other("/").headers()[header::CACHE_CONTROL], "no-store");
    }
}
