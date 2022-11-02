use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use tokio::sync::OnceCell;
use uuid::Uuid;

use crate::job_manager::task_scheduler::Segment;
use crate::jobs::Job;
use crate::storage::FileRef;

enum AllocationState {
	Queue = 0,
	Allocated = 1,
	Completed = 2,
}

struct SegmentData {
	segment: Segment,
	state: AtomicU8,
	job_id: Uuid,
	output: OnceCell<FileRef>,
}

impl SegmentData {
	fn allocate(self: &Arc<Self>) -> Option<Arc<Self>> {
		self.state
			.compare_exchange(
				AllocationState::Queue as u8,
				AllocationState::Allocated as u8,
				Ordering::SeqCst,
				Ordering::Acquire,
			)
			.ok()
			.and(Some(self.clone()))
	}
	fn complete(self: &Arc<Self>) {
		self.state
			.store(AllocationState::Completed as u8, Ordering::Release)
	}
}

///Hold segment data
pub(super) struct SegmentEntry(Arc<SegmentData>);

impl SegmentEntry {
	fn allocate(&self) -> Option<SegmentAllocation> {
		self.0.allocate().map(SegmentAllocation)
	}
	pub fn get_output(&self) -> Option<FileRef> {
		self.0.output.get().cloned()
	}
	pub fn segment_number(&self) -> usize {
		0
	}
}

///RAII wrapper for Segment allocation
pub(super) struct SegmentAllocation(Arc<SegmentData>);

impl SegmentAllocation {
	pub fn as_segment(&self) -> &Segment {
		&self.0.segment
	}
	pub fn job_id(&self) -> &Uuid {
		&self.0.job_id
	}
	pub fn set_completed(self) -> SegmentEntry {
		self.0.complete();
		SegmentEntry(self.0.clone()) //Cannot move out because of drop()
	}
	pub fn set_output(&self, output: FileRef) -> Result<(), ()> {
		self.0.output.set(output).map_err(|_| ())
	}
}

impl Drop for SegmentAllocation {
	fn drop(&mut self) {
		//Like C++ compare_exchange_strong, this is required not to fail if comparison succeeds
		let _ = self.0.state.compare_exchange(
			AllocationState::Allocated as u8,
			AllocationState::Queue as u8,
			Ordering::SeqCst,
			Ordering::Acquire,
		);
	}
}

pub(super) struct JobSegmenter {
	job: Arc<Job>,
	job_id: Uuid,
	segments: OnceCell<SegmentEntry>,
}

impl JobSegmenter {
	pub(super) fn new(job: Arc<Job>, job_id: Uuid) -> Self {
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

	pub fn get_segment(&self, segment_number: usize) -> Option<SegmentEntry> {
		(segment_number == 0).then_some(()).and_then(|()| {
			self.segments
				.get()
				.map(|entry| SegmentEntry(entry.0.clone()))
		})
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
			output: OnceCell::new(),
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

	#[test]
	fn set_complete_return_segment_number() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = JobSegmenter::new(job, job_uuid);

		let available = segmenter.get_available().unwrap();
		//Only check type at compile time
		let _segment_number: usize = available.set_completed().segment_number();
	}

	#[test]
	fn segment_entry_got_with_segment_number_should_be_equivalent_to_allocated() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = JobSegmenter::new(job, job_uuid);

		let available = segmenter.get_available().unwrap();
		let available = available.set_completed();
		let segment_number: usize = available.segment_number();
		let entry = segmenter.get_segment(segment_number).unwrap();
		assert!(
			Arc::ptr_eq(&entry.0, &available.0),
			"Both pointers should be equivalent"
		)
	}

	#[test]
	fn get_segment_with_invalid_number_returns_none() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = JobSegmenter::new(job, job_uuid);

		let available = segmenter.get_available().unwrap();
		let segment_number: usize = available.set_completed().segment_number();
		let entry = segmenter.get_segment(segment_number + 1);
		assert!(entry.is_none())
	}

	#[test]
	fn after_drop_complete_should_not_be_able_to_allocate() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = JobSegmenter::new(job, job_uuid);

		let available = segmenter.get_available().unwrap();
		let available = available.set_completed();
		drop(available);
		let available = segmenter.get_available();
		assert!(available.is_none())
	}

	#[test]
	fn get_segment_output_returns_none() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = JobSegmenter::new(job, job_uuid);

		let available = segmenter.get_available().unwrap();
		let segment_number: usize = available.set_completed().segment_number();
		let entry = segmenter.get_segment(segment_number).unwrap();

		let output = entry.get_output();
		assert!(output.is_none())
	}

	#[test]
	fn get_segment_output_after_set_output_returns_the_same_value_from_set() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = JobSegmenter::new(job, job_uuid);

		let output = FileRef::from(Uuid::new_v4());

		let available = segmenter.get_available().unwrap();
		available.set_output(output.clone()).unwrap();
		let segment_number: usize = available.set_completed().segment_number();
		let entry = segmenter.get_segment(segment_number).unwrap();

		let got_output = entry.get_output().unwrap();
		assert_eq!(got_output, output);
	}

	#[test]
	fn segment_set_output_second_time_fails() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = JobSegmenter::new(job, job_uuid);

		let output = FileRef::from(Uuid::new_v4());
		let output2 = FileRef::from(Uuid::new_v4());

		let available = segmenter.get_available().unwrap();
		available.set_output(output).unwrap();
		let res = available.set_output(output2);
		assert!(res.is_err());
	}
}
