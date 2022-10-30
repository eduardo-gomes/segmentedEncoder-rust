use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use tokio::sync::OnceCell;
use uuid::Uuid;

use crate::job_manager::task_scheduler::Segment;
use crate::jobs::Job;

pub(super) struct JobSegmenter {
	job: Arc<Job>,
	job_id: Uuid,
	segments: OnceCell<SegmentEntry>,
}

struct SegmentData {
	segment: Segment,
	state: AtomicU8,
	job_id: Uuid,
}

///Hold segment data
struct SegmentEntry(Arc<SegmentData>);

///RAII wrapper for Segment allocation
pub(super) struct SegmentAllocation(Arc<SegmentData>);

enum AllocationState {
	Queue = 0,
	Allocated = 1,
	Completed = 2,
}

impl SegmentAllocation {
	pub fn as_segment(&self) -> &Segment {
		&self.0.segment
	}
	pub fn job_id(&self) -> &Uuid {
		&self.0.job_id
	}
	pub fn set_completed(&self) -> usize {
		let state = &self.0.state;
		state.store(AllocationState::Completed as u8, Ordering::Release);
		0
	}
}

impl Drop for SegmentAllocation {
	fn drop(&mut self) {
		let _ = self.0.state.compare_exchange(
			AllocationState::Allocated as u8,
			AllocationState::Queue as u8,
			Ordering::SeqCst,
			Ordering::Acquire,
		);
	}
}

impl SegmentEntry {
	fn allocate(&self) -> Option<SegmentAllocation> {
		let state = &self.0.state;
		let allocate = state.compare_exchange(
			AllocationState::Queue as u8,
			AllocationState::Allocated as u8,
			Ordering::SeqCst,
			Ordering::Acquire,
		);
		allocate.map(|_| SegmentAllocation(self.0.clone())).ok()
	}
}

impl JobSegmenter {
	pub fn new(job: Arc<Job>, job_id: Uuid) -> Self {
		Self {
			job,
			job_id,
			segments: OnceCell::new(),
		}
	}
	pub fn get_available(&self) -> Option<SegmentAllocation> {
		self.segments
			.get()
			.or_else(|| self.next_segment())
			.and_then(|segment| segment.allocate())
	}
}

impl JobSegmenter {
	///Internal function to segment jobs.
	///
	///This may differ for different kinds of segmentation.
	fn next_segment(&self) -> Option<&SegmentEntry> {
		let job_id = self.job_id;
		let segment = Segment {
			input_path: format!("/api/jobs/{}/source", job_id),
			parameters: self.job.parameters.clone(),
		};
		let segment = Arc::new(SegmentData {
			segment,
			state: AtomicU8::new(AllocationState::Queue as u8),
			job_id,
		});
		self.segments
			.set(SegmentEntry(segment))
			.ok()
			.and_then(|()| self.segments.get())
	}
}

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use uuid::Uuid;

	use crate::job_manager::task_scheduler::job_segmenter::JobSegmenter;
	use crate::jobs::{Job, JobParams, Source};
	use crate::storage::FileRef;

	#[test]
	fn job_segmenter_get_available_return_some_for_do_not_segment() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = JobSegmenter::new(job, job_uuid);

		let available = segmenter.get_available();
		assert!(available.is_some())
	}

	#[test]
	fn job_segmenter_get_available_twice_return_none() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = JobSegmenter::new(job, job_uuid);

		let _available = segmenter.get_available();
		let available = segmenter.get_available();
		assert!(available.is_none())
	}

	#[test]
	fn job_segmenter_after_drop_can_allocate_again() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = JobSegmenter::new(job, job_uuid);

		let available = segmenter.get_available();
		drop(available);
		let available = segmenter.get_available();
		assert!(available.is_some())
	}
}
