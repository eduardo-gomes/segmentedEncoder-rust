pub(crate) mod ranged {
	use axum::response::{IntoResponse, Response};
	use axum_extra::headers::Range;
	use axum_range::{KnownSize, Ranged};
	use tokio::io::{AsyncRead, AsyncSeek};

	pub(crate) async fn from_reader<T: AsyncRead + AsyncSeek + Send + Unpin + 'static>(
		read: T,
		range: Option<Range>,
	) -> std::io::Result<Result<Response, Response>> {
		let known_size = KnownSize::seek(read).await?;
		Ok(Ranged::new(range, known_size)
			.try_respond()
			.map(|res| res.into_response())
			.map_err(|res| res.into_response()))
	}

	#[cfg(test)]
	mod test {
		use std::io::Cursor;

		use axum::body::to_bytes;
		use axum_extra::headers::Range;

		use crate::api::utils::ranged::from_reader;
		use crate::WEBM_SAMPLE;

		#[tokio::test]
		async fn with_no_option_returns_entire_content() {
			let content = Cursor::new(WEBM_SAMPLE);
			let body = from_reader(content, None)
				.await
				.unwrap()
				.unwrap()
				.into_body();
			let bytes = to_bytes(body, WEBM_SAMPLE.len() + 10).await.unwrap();
			assert_eq!(bytes, WEBM_SAMPLE.as_slice())
		}

		#[tokio::test]
		async fn with_range_return_the_selected_range() {
			let content = Cursor::new(WEBM_SAMPLE);
			let body = from_reader(content, Some(Range::bytes(0..10).unwrap()))
				.await
				.unwrap()
				.unwrap()
				.into_body();
			let bytes = to_bytes(body, WEBM_SAMPLE.len() + 10).await.unwrap();
			assert_eq!(bytes.as_ref(), &WEBM_SAMPLE[0..10])
		}

		#[tokio::test]
		async fn with_bad_range_ok_error() {
			let content = Cursor::new(WEBM_SAMPLE);
			let len = WEBM_SAMPLE.len();
			let res = from_reader(content, Some(Range::bytes(len as u64..).unwrap()))
				.await
				.unwrap();
			assert!(res.is_err())
		}
	}
}

pub(crate) mod parse {
	use axum::http::header::ToStrError;
	use axum::http::{HeaderMap, HeaderValue};

	use task::{JobOptions, Options};

	pub fn parse_job_options(headers: &HeaderMap) -> Result<JobOptions, ToStrError> {
		let video_codec = headers
			.get("video_codec")
			.map(|val| val.to_str())
			.transpose()?
			.map(String::from);
		let video_params = split_multiple_headers_into_strings(headers.get_all("video_param"))?;
		Ok(JobOptions {
			video: Options {
				codec: video_codec,
				params: video_params,
			},
			audio: None,
		})
	}

	pub fn split_multiple_headers_into_strings<'a, I>(iter: I) -> Result<Vec<String>, ToStrError>
	where
		I: IntoIterator<Item = &'a HeaderValue>,
	{
		iter.into_iter()
			.map(|val| val.to_str().map(|str| str.split(',').map(String::from)))
			.collect::<Result<Vec<_>, _>>()
			.map(|vec| vec.into_iter().flatten().collect())
	}

	#[cfg(test)]
	mod test {
		use axum::http::{HeaderMap, HeaderValue};

		use crate::api::utils::parse::{parse_job_options, split_multiple_headers_into_strings};

		#[test]
		fn with_empty_iterator_return_empty_vec() {
			let res = split_multiple_headers_into_strings(vec![]).unwrap();
			assert!(res.is_empty())
		}

		#[test]
		fn with_single_simple_value_returns_the_value() {
			let src = "simple";
			let value = HeaderValue::from_static(src);
			let res = split_multiple_headers_into_strings(vec![&value]).unwrap();
			assert_eq!(res.first().unwrap().as_str(), src)
		}

		#[test]
		fn with_two_simple_values_returns_the_two_values() {
			let src = ["first", "second"];
			let values = [
				HeaderValue::from_static(src[0]),
				HeaderValue::from_static(src[1]),
			];
			let res = split_multiple_headers_into_strings(values.as_ref()).unwrap();
			assert_eq!(res[0].as_str(), src[0]);
			assert_eq!(res[1].as_str(), src[1]);
		}

		#[test]
		fn with_two_comma_separated_values_returns_the_two_values() {
			let src = ["first", "second"];
			let values = HeaderValue::from_str(src.join(",").as_str()).unwrap();
			let res = split_multiple_headers_into_strings([&values]).unwrap();
			assert_eq!(res[0].as_str(), src[0]);
			assert_eq!(res[1].as_str(), src[1]);
		}

		#[test]
		fn parse_video_codec_job_options() {
			let codec = "libx264";
			let mut headers = HeaderMap::new();
			headers.insert("video_codec", HeaderValue::from_static(codec));
			let options = parse_job_options(&headers).unwrap();
			assert_eq!(options.video.codec.unwrap().as_str(), codec)
		}

		#[test]
		fn parse_video_args_job_options() {
			let args = ["-preset", "ultrafast"];
			let mut headers = HeaderMap::new();
			headers.append("video_param", HeaderValue::from_static(args[0]));
			headers.append("video_param", HeaderValue::from_static(args[1]));
			let params = parse_job_options(&headers).unwrap().video.params;
			assert_eq!(
				params,
				args.into_iter().map(String::from).collect::<Vec<_>>()
			);
		}
	}
}
