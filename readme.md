# Segmented Encoder

A tool to transcode video on multiple computers in parallel.

It will split a video into lots of small segments. Each segment will be assigned to an available worker. The server will
have a pool of pending tasks.
Each client will get one task, and if necessary will fetch the video, then transcode and upload back to the server.

Since gRPC uses HTTP, the server will also use it to make the files available.
Also, we could provide an API with a web interface to manage the server.

# Building

## Protocol Buffers

This application depends on `protoc` to compile.
You can use the `protobuf-compiler` package on Debian or similar for your system or manually install and add to PATH

## Client interface

The client will access a gRPC interface to schedule tasks and report status.

### Interface methods

- registerClient(RegistrationRequest) returns (RegistrationResponse)
- getWorkerRegistration(Empty) returns (RegistrationResponse)
- requestTask(Empty) returns (Task)

## HTTP client interface

This interface will be used to download input files, and upload output

- /api/jobs/{job_id}/tasks/{task_id}/input/{num}
- /api/jobs/{job_id}/tasks/{task_id}/output

## Worker interaction

The worker, after authentication, will contact the server through a few methods.
First, it will request a task, then it will start the task using the HTTP endpoint to get the input. And while the task
is running, the worker will send status updates periodically, and inform the server after the task is successfully
finished. Another HTTP endpoint will be used to send the task output to the server, and the task will only be finished
after the output is transferred and the worker tell the server.

## task types

Each job may have multiple tasks.

To work as intended, at least 3 kind of tasks are needed.

- Analysis task
- Transcode task
- Merge task

### Analysis

This kind of task will get all the information needed to create the necessary transcode tasks.

### Merge

This task will get the output from the other tasks ang generate the final result