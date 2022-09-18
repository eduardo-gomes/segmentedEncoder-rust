# Segmented Encoder

A tool to transcode video on multiple computers in parallel.

It will split a video into lots of small segments. Each segment will be assigned to an available worker. The server will
have a pool of pending tasks.
Each client will get one task, and if necessary will fetch the video, then transcode and upload back to the server.

Since gRPC uses HTTP, the server will also use it to make the files available.
Also, we could provide an API with a web interface to manage the server.
