## Config

#### Unit (12 s) Per (1) Budget(12 s) Jobs(142)

- budget: 12000
- unit: 12000
- per: 1
- max: 1500000

#### Unit (10 s) Per (2) Budget(5 s) Jobs(160)

- budget: 5000
- unit: 10000
- per: 2
- max: 650000

#### Unit (20 s) Per (4) Budget(5 s) Jobs(184)

- budget: 5000
- unit: 20000
- per: 4
- max: 650000

## CSV Files

- Under `./data`
- **jobs.csv**     
  - original file after running `cargo run -- run fifo`
- **req.csv**      
  - extract *Admitted* and *Deadline* from `jobs.csv` and add request sentence from `news.en`, used as the input file for `async_dynamic_test.py`
- **output.csv**   
  - output file after running `async_dynamic_test.py`
- **final_result.csv**
  - final organized file with *Admitted, Deadline, Started, Finished, State and Latency*

## Plots

- Under `./plots`

## Experiment Setting

- **Model:** Facebook/Mbart50 

   https://huggingface.co/transformers/model_doc/mbart.html?highlight=mbart50

- **Dataset:** WMT2018 News Commentary

   [http://statmt.org/wmt18/translation-task.html#download](http://statmt.org/wmt18/translation-task.html) 

- **Language:** en2zh