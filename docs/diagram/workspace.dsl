workspace {
	model {
		user = person User
		se = softwareSystem SegmentedEncoder {
			spa = container "Single Page Application"
			api = container "Rest API" {
				authApi = component "Auth" "Check auth"
				jobApi = component "Job API" "Manage jobs"
				fileApi = component "File API" "Store and retrive files"
			}
			authDb = container "Auth Database" "Store authentication data"
			jobDb = container "Job Database" "Store job information"
			fs = container "File system" "Stores files to process and results"
			rpc = container "gRPC API" {
				jobAlocator = component "Job allocator" "Get available jobs and set the required permisions for the client"
				statusUpdater = component "Status update" "Track job progress"
			}
		}
		worker = softwareSystem "SegmentedEncoder Client"

		user -> spa "Send files and get results from"
		spa -> api "Makes API calls to"
		spa -> jobApi "Send job to"

		authApi -> authDb "Reads from"
		jobApi -> authApi "Verify"
		jobApi -> fs "Send source file"
		jobApi -> jobDb "Read and write to"
		fileApi -> authApi "Verify"
		fileApi -> fs "Read and write to"

		jobAlocator -> jobDb "Read and write to"
		jobAlocator -> authDb "Read and write to"
		statusUpdater -> jobDb "Write status to"
		statusUpdater -> authDb "Read from"

		worker -> jobAlocator "Get jobs from"
		worker -> statusUpdater "Send updates to"
		worker -> fileApi "Get and send files to"
	}

	views {
		systemContext se "diagram1" {
			include *
			autolayout lr
		}

		container se "diagram2" {
			include *
			autolayout lr
		}

		component api "diagram3" {
			include *
			autolayout lr
		}
		component rpc "diagram4" {
			include *
			autolayout lr
		}
	}
}