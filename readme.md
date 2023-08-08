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
