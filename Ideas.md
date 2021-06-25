# Algorithm

Push execution as late as possible, while not affecting later ones

look-ahead window: currently unbounded - whatever available in the pending queue is considered.

Later if performance becomes a problem, we can revisit and bound the window to a certain size.

## Design A: compute backwards
This doesn't work because the last half of requests may be disconnected with the former half.
And the former half can very well be bounded by itself, without any connection to the last half.

Once get hold of a window (a bag of jobs)
- sort their budget intervals,
- start to form batches backwards according to worker batch size
- until the first batch
    - if the first batch is full, or waken up by alarm, run it
    - else wait for next event (new job, batch done, or alarm)

This has a lot of re-computation, but is mostly liner. A future optimization would be to reuse previous information to seed to current one.

## Design B: push
Think each request as a spring and its feasible (budget - p99) interval as a slot holding the spring.
Push the first request until not possible, take first consecutive full batches and wake up later.

While pushing, obey the following constrains

- request can not start earlier than push point
- request coming in late can not start earlier than previous requests
- request can not start earlier than full batch done point
- request can not start later than its feasible interval
- request tends to start early (think of a spring)

The algorithm is

- try to move the push point to next step (maybe 0.1 in time? or whatever resolution)
- for each request in order
  - try to adjust the request to obey all constrains by pushing it later
  - if not possible
    - break, and revert the push point move
  - else: continue
- take the first consecutive full batches and submit according to their time
