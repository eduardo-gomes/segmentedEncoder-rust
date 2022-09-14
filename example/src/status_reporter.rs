use std::task::{Poll, Waker};
use std::time::{Duration, Instant};

use futures_util::Stream;

use super::status::{Duration as StatusDuration, Progress, Stage, StatusReport};

impl From<Duration> for StatusDuration {
	fn from(duration: Duration) -> Self {
		StatusDuration {
			seconds: duration.as_secs(),
			nanoseconds: duration.subsec_nanos(),
		}
	}
}

/// Essa classe gera uma série de StatusReport com um intervalo entre eles,
/// simulando o que uma tarefa faria.
pub(super) struct DelayedStatusReporterStream {
	len: i32,
	at: i32,
	delay: u64,
	ready_at: Instant,
	started_at: Instant,
}

impl DelayedStatusReporterStream {
	/// delay in milliseconds
	pub(super) fn new(delay: u64, len: i32) -> Self {
		DelayedStatusReporterStream {
			len,
			at: 0,
			delay,
			ready_at: Instant::now(),
			started_at: Instant::now(),
		}
	}
	fn gen_and_unready(&mut self) -> StatusReport {
		let at = self.at;
		self.at += 1;
		let now = Instant::now();
		let elapsed = (now - self.started_at).into();
		if self.len >= at {
			if self.len > at {
				// Não aguarda após concluir, assim, o report com Finished é enviado em seguida
				self.ready_at = now + Duration::from_millis(self.delay);
			}

			StatusReport {
				phase: Stage::Working.into(),
				elapsed: Some(elapsed),
				progress: Some(Progress {
					numerator: at,
					denominator: self.len,
				}),
				report_string: "".into(),
			}
		} else {
			StatusReport {
				phase: Stage::Finished.into(),
				progress: None,
				elapsed: Some(elapsed),
				report_string: "Delayed finished".into(),
			}
		}
	}
	fn is_ready(&self) -> bool {
		Instant::now() >= self.ready_at
	}
	fn notify_waker_on_ready(&self, waker: &Waker) {
		let ready_at = self.ready_at.into();
		let waker = waker.clone();
		tokio::spawn(async move {
			tokio::time::sleep_until(ready_at).await;
			waker.wake();
		});
	}
	fn remaining(&self) -> usize {
		let signed = self.len - self.at + 2; // +1 because is closed interval [0, len], +1 FINISHED
		signed.try_into().unwrap_or_default()
	}
}

impl Stream for DelayedStatusReporterStream {
	type Item = StatusReport;

	fn poll_next(
		self: std::pin::Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Option<Self::Item>> {
		if self.remaining() <= 0 {
			Poll::Ready(None)
		} else {
			match self.is_ready() {
				true => std::task::Poll::Ready(Some(self.get_mut().gen_and_unready())),
				false => {
					self.notify_waker_on_ready(cx.waker());
					std::task::Poll::Pending
				}
			}
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let remaining: usize = (self.len - self.at).try_into().unwrap();
		(remaining, Some(remaining))
	}
}
