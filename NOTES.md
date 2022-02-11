TESTING DESIGN
==============

I am seeing wild and confusing results around throughput of various event loop constructions:

* First, I can't get consistent perf numbers under macOS _at all_ - things dance around 4x (I've seen the same configuration do 25k RPS throughput and 100k RPS throughput)
* macOS seems 2x slower than a Linux VM on the same hardware with fewer cores (4 cores dedicated to linux, 200-220k RPS).  Is this due to benchmark wonkiness around efficiency cores (see preceding bullet point), or is kqueue+tokio slower (or not optimized as much) than epoll+tokio?
* multi-threaded event loop vs. `$NCPU` single-threaded event loops
  * on my Intel laptop, 40% performance improvement moving to multiple single-threaded event loops (each with their own `SO_REUSEPORT` listener sockets).
  * on a Linux VM on M1X, its either a wash or threadpool is actually slightly _faster_
  * On linux VM, the fastest I've seen is actually _one_ single-threaded event loop
    * why is this? is it malloc/free global contention?

Current benchmark we are talking about is:
* `hey` (Go statically linked binary built with 1.18 tip) -> `upstream-service` axum hello-world HTTP (non-HTTPS) server

I think I'd learn a lot setting up a benchmark harness to test the following:
* event loop vs. $n single-threaded event loops vs. $n processes each with single-threaded event loops
* cross the above with allocators - some may do significantly better or worse in the presence or absence of a thread pool
  * (A thread pool seems like it will have the behavior that very frequently e.g. a HashMap of headers for a request is allocated on one thread and freed on another.  For some allocators, this is a pathalogical case where per-thread caches effectively stop working). 

A big thing here will be ensuring the load generating tool doesn't interfere with the HTTP servers -- probably need to do something like partition $NCPUs into 2, give half to the benchmark harness and the rest to the server.  
Or carve out 2 CPUs for harness, assume thats good enough and give the rest to the server.

