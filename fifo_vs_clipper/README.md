## Compare FIFO and Clipper

### Command

1. `cargo run -- run fifo `
2. run the `compare_fifo_clipper.ipynb` under `fifo_vs_clipper`

### Config

#### **Average** **Length** **(100) Total jobs (480) Batch Size (20)**

- lambda: 8

- offset: 80
- factor: 80
- budget: 130/260/390
- unit: 250
- per: 5s
- max: 20000

#### **Average** **Length** **(200) Total jobs (480) Batch Size (20)**

- lambda: 6

- offset: 160
- factor: 100
- budget: 230/460/690
- unit: 500
- per: 5
- max: 40000

### Plots

- Please see under `fifo_vs_clipper/plots`
- They are also presented in `fifo_vs_clipper/result.pptx`

### Problem

I make a little change to the `async_dynamic_test.py `. In the function `incoming_file`, I add one more `yield batch, delay_ms` to launch the final batch. However, this final batch always gets error after running the async_dynamic_test, which is shown in the `log/bs20/clipper/latency.csv`.


