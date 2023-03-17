//! Module to segment jobs
//!
//! [Segmenter] will generate tasks(TaskInfo) and subjobs for [JobScheduler]
use crate::jobs::Job;

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) enum Segmenter {
	DoNotSegment,
}
