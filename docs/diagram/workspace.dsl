workspace {
	model {
		user = person User
		se = softwareSystem "SegmentedEncoder" {
			spa = container "Single Page Application" {
				tags "Web App"
			}
			group "Server" {
				api = container "Rest API" {
					authApi = component "Auth" "Check auth"
					jobApi = component "Job API" "Manage jobs"
					fileApi = component "File API" "Store and retrive files"
				}
				authDb = container "Auth Database" "Store authentication data" {
					tags "Database"
				}
				jobDb = container "Job Database" "Store job information" {
					tags "Database"
				}
				rpc = container "gRPC API" {
					jobAlocator = component "Job allocator" "Get available jobs and set the required permisions for the client"
					statusUpdater = component "Status update" "Track job progress"
				}
			}
			fs = container "File system" "Stores files to process and results" {
				tags "File System"
			}
			group "Worker" {
				ffmpeg = container "FFmpeg"
				client = container "Worker Client" {
					runner = component "Job runner" "Requests and execute jobs"
					reporter = component "Status Reporter"
				}
			}
		}

		user -> spa "Send files and get results from"
		spa -> api "Makes API calls to" "HTTP"
		spa -> jobApi "Send job to" "HTTP"

		authApi -> authDb "Reads from"
		jobApi -> authApi "Verify"
		jobApi -> fs "Send source file to"
		jobApi -> jobDb "Read and write to"
		fileApi -> authApi "Verify"
		fileApi -> fs "Read and write to"

		jobAlocator -> jobDb "Read and write to"
		jobAlocator -> authDb "Read and write to"
		statusUpdater -> jobDb "Write status to"
		statusUpdater -> authDb "Read from"

		# Worker relationships
		runner -> jobAlocator "Get jobs from" "gRPC"
		runner -> ffmpeg "Runs jobs with"
		reporter -> ffmpeg "Track status from"
		reporter -> statusUpdater "Send updates to"
		ffmpeg -> fileApi "Get and send files to" "HTTP"
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

		component client "diagram5" {
			include *
		}

		styles {
			element "Person" {
				background #08427b
				color white
				shape Person
			}

			element "Software System" {
				background #1168bd
				color white
			}

			element "Container" {
				background #438dd5
				color white
			}

			element "Component" {
				background #85bbf0
				color black
			}

			element "File System" {
				shape Folder
			}

			element "Database" {
				shape Cylinder
			}

			element "Web App" {
				shape WebBrowser
			}
		}
	}
}