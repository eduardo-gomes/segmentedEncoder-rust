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
