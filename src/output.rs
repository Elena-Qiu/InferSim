use std::fs::File;
use std::io;
use std::io::{BufWriter, Write};

use serde_json::json;

use crate::config::AppConfigExt as _;
use crate::sim::{msg, Event, Message};
use crate::types::{Duration, Job, Time};
use crate::utils::prelude::*;
use crate::SimConfig;

fn event_line(writer: impl io::Write, val: serde_json::Value) -> Result<()> {
    event_line_with_ending(writer, val, true)
}

fn event_line_with_ending(mut writer: impl io::Write, val: serde_json::Value, ending: bool) -> Result<()> {
    serde_json::to_writer(&mut writer, &val).kind(ErrorKind::ChromeTracing)?;
    if ending {
        writer
            .write_all(b",\n")
            .kind(ErrorKind::ChromeTracing)?;
    }
    Ok(())
}

pub fn render_chrome_trace<'a, E>(events: E) -> Result<()>
where
    E: IntoIterator<Item = &'a Event>,
{
    const BATCH_PID: usize = 100;

    let path = config().output_dir()?.file("timeline.json")?;
    info!("writing chrome trace to {}", path.display());

    let mut file = BufWriter::new(File::create(&path).kind(ErrorKind::ChromeTracing)?);
    file.write_all(b"{\"traceEvents\":[\n")
        .kind(ErrorKind::ChromeTracing)?;

    let mut past_due = 0;
    event_line(
        &mut file,
        json!({
            "name": "Past Due Jobs",
            "ph": "C",
            "cat": "past_due",
            "ts": 0,
            "pid": 0,
            "args": {
                "past_due": past_due,
            }
        }),
    )?;

    for evt in events {
        let time = evt.time;
        match &evt.message {
            Message::IncomingJobs(msg::IncomingJobs { jobs, .. }) => {
                for job in jobs.iter() {
                    // infer time are drawn as duration on pid 1
                    event_line(
                        &mut file,
                        json!({
                            "name": format!("Job {}", job.id),
                            "ph": "X",
                            "cat": "exec.exact",
                            "ts": time,
                            "dur": job.length.value(),
                            "id": job.id,
                            "tid": job.id,
                            "pid": 1,
                            "args": {
                                "job_id": job.id,
                                "job_len": job.length.value(),
                            }
                        }),
                    )?;
                    // queuing time are drawn as duration on pid 0
                    event_line(
                        &mut file,
                        json!({
                            "name": format!("Job {}", job.id),
                            "ph": "B",
                            "cat": "queuing",
                            "ts": time,
                            "id": job.id,
                            "tid": job.id,
                            "pid": 0,
                            "args": {
                                "job_id": job.id,
                                "job_len": job.length.value(),
                            }
                        }),
                    )?;
                    if let Some(budget) = job.budget {
                        event_line(
                            &mut file,
                            json!({
                                "name": format!("Job {} Deadline", job.id),
                                "ph": "s",
                                "cat": "deadline.flow",
                                "ts": time,
                                "id": job.id,
                                "tid": job.id,
                                "pid": 0,
                                "args": {
                                    "job_id": job.id,
                                }
                            }),
                        )?;
                        event_line(
                            &mut file,
                            json!({
                                "name": format!("Job {} Deadline", job.id),
                                "ph": "f",
                                "cat": "deadline.flow",
                                "ts": time + budget,
                                "id": job.id,
                                "tid": job.id,
                                "pid": 0,
                                "args": {
                                    "job_id": job.id,
                                }
                            }),
                        )?;
                        event_line(
                            &mut file,
                            json!({
                                "name": format!("Job {} Deadline", job.id),
                                "ph": "I",
                                "cat": "deadline",
                                "ts": time + budget,
                                "id": job.id,
                                "tid": job.id,
                                "pid": 0,
                                "args": {
                                    "job_id": job.id,
                                }
                            }),
                        )?;
                    }
                }
            }
            Message::BatchDone(msg::BatchDone { batch }) => {
                // the whole batch span
                event_line(
                    &mut file,
                    json!({
                        "name": "Batch",
                        "ph": "X",
                        "cat": "exec.batch",
                        "ts": batch.started(),
                        "dur": time - batch.started(),
                        "tid": 0,
                        "pid": BATCH_PID + batch.id,
                        "args": {
                            "batch_size": batch.done.len() + batch.past_due.len(),
                        }
                    }),
                )?;
                for (idx, job) in batch.done.iter().enumerate() {
                    let idx = idx + 1;
                    // flow arrow start
                    event_line(
                        &mut file,
                        json!({
                            "name": format!("Job {}", job.id),
                            "ph": "s",
                            "cat": "scheduling",
                            "ts": batch.started(),
                            "id": job.id,
                            "tid": job.id,
                            "pid": 0,
                            "args": {
                                "job_id": job.id,
                            }
                        }),
                    )?;
                    // close queuing span
                    event_line(
                        &mut file,
                        json!({
                            "name": format!("Job {}", job.id),
                            "ph": "E",
                            "cat": "queuing",
                            "ts": batch.started(),
                            "id": job.id,
                            "tid": job.id,
                            "pid": 0,
                            "args": {
                                "job_id": job.id,
                            }
                        }),
                    )?;
                    // flow arrow target
                    event_line(
                        &mut file,
                        json!({
                            "name": format!("Job {}", job.id),
                            "ph": "f",
                            "bp": "e",
                            "cat": "scheduling",
                            "ts": batch.started() + Duration(0.01),
                            "id": job.id,
                            "tid": idx,
                            "pid": BATCH_PID + batch.id,
                            "args": {
                                "job_id": job.id,
                            }
                        }),
                    )?;
                    // execution
                    event_line(
                        &mut file,
                        json!({
                            "name": format!("Job {}", job.id),
                            "ph": "X",
                            "cat": "exec",
                            "ts": batch.started(),
                            "dur": job.length.value(),
                            "id": job.id,
                            "tid": idx,
                            "pid": BATCH_PID + batch.id,
                            "args": {
                                "job_id": job.id,
                            }
                        }),
                    )?;
                }
                past_due = chrome_trace_past_due(&mut file, past_due, &evt.time, &batch.past_due)?;
            }
            Message::PastDue(msg::PastDue { jobs }) => {
                past_due = chrome_trace_past_due(&mut file, past_due, &evt.time, jobs)?;
            }
            _ => (),
        }
    }

    event_line(
        &mut file,
        json!({
            "name": "process_name",
            "ph": "M",
            "pid": 1,
            "args": {
                "name": "Pending Jobs (Exact Inference Time)"
            }
        }),
    )?;
    event_line(
        &mut file,
        json!({
            "name": "process_sort_index",
            "ph": "M",
            "pid": 1,
            "args": {
                "sort_index": 1
            }
        }),
    )?;
    event_line(
        &mut file,
        json!({
            "name": "process_name",
            "ph": "M",
            "pid": 0,
            "args": {
                "name": "Pending Jobs (Waiting Time)"
            }
        }),
    )?;
    event_line(
        &mut file,
        json!({
            "name": "process_sort_index",
            "ph": "M",
            "pid": 0,
            "args": {
                "sort_index": 0
            }
        }),
    )?;
    event_line(
        &mut file,
        json!({
            "name": "process_sort_index",
            "ph": "M",
            "pid": BATCH_PID,
            "args": {
                "sort_index": 100
            }
        }),
    )?;
    event_line_with_ending(
        &mut file,
        json!({
            "name": "process_name",
            "ph": "M",
            "pid": BATCH_PID,
            "args": {
                "name": "Batch 0"
            }
        }),
        false,
    )?;
    file.write_all(b"\n],\"config\":")
        .kind(ErrorKind::ChromeTracing)?;

    let cfg: SimConfig = config().fetch()?;
    serde_json::to_writer(&mut file, &cfg).kind(ErrorKind::ChromeTracing)?;
    file.write_all(b"\n}")
        .kind(ErrorKind::ChromeTracing)?;
    Ok(())
}

fn chrome_trace_past_due(
    mut file: &mut BufWriter<File>,
    past_due: usize,
    time: &Time,
    jobs: &Vec<Job>,
) -> Result<usize> {
    for job in jobs.iter() {
        // finish the queuing span
        event_line(
            &mut file,
            json!({
                "name": format!("Job {}", job.id),
                "ph": "E",
                "cat": "queuing",
                "ts": time,
                "id": job.id,
                "tid": 0,
                "pid": 0,
                "args": {
                    "job_id": job.id,
                }
            }),
        )?;
    }
    // update the counter
    event_line(
        &mut file,
        json!({
            "name": "Past Due Jobs",
            "ph": "C",
            "cat": "past_due",
            "ts": time,
            "pid": 0,
            "args": {
                "past_due": past_due,
            }
        }),
    )?;
    Ok(past_due + jobs.len())
}

pub fn render_job_trace<'a, E>(events: E) -> Result<()>
where
    E: IntoIterator<Item = &'a Event>,
{
    let path = config().output_dir()?.file("jobs.csv")?;
    info!("writing job trace to {}", path.display());

    #[derive(serde::Serialize)]
    #[serde(rename_all = "snake_case")]
    enum State {
        PastDue,
        Done,
    }
    #[derive(serde::Serialize)]
    #[serde(rename_all = "PascalCase")]
    struct Row {
        job_id: usize,
        length: Duration,
        length_p99: Duration,
        admitted: Time,
        deadline: Option<Time>,
        started: Option<Time>,
        finished: Option<Time>,
        state: State,
    }

    fn row(job: &Job, started: impl Into<Option<Time>>, finished: impl Into<Option<Time>>, state: State) -> Row {
        let (started, finished) = (started.into(), finished.into());
        if started.is_some() != finished.is_some() {
            unreachable!("started and finished has to be in sync");
        }
        Row {
            job_id: job.id,
            length: job.length.value(),
            length_p99: job.length.quantile(0.99),
            admitted: job.admitted,
            deadline: job.deadline,
            started,
            finished,
            state,
        }
    }

    let mut writer = csv::Writer::from_path(path).kind(ErrorKind::JobsCsv)?;

    for evt in events {
        let time = evt.time;
        match &evt.message {
            Message::BatchDone(msg::BatchDone { batch }) => {
                for job in batch.done.iter() {
                    writer
                        .serialize(row(job, batch.started(), time, State::Done))
                        .kind(ErrorKind::JobsCsv)?;
                }
                for job in batch.past_due.iter() {
                    writer
                        .serialize(row(job, batch.started(), time, State::PastDue))
                        .kind(ErrorKind::JobsCsv)?;
                }
            }
            Message::PastDue(msg::PastDue { jobs }) => {
                for job in jobs.iter() {
                    writer
                        .serialize(row(job, None, None, State::PastDue))
                        .kind(ErrorKind::JobsCsv)?;
                }
            }
            _ => (),
        }
    }
    Ok(())
}
