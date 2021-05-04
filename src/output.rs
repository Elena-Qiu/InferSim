use std::fs::File;
use std::io;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use desim::Event;
use serde_json::json;

use crate::simulator::SystemState;
use crate::utils::prelude::*;
use crate::SimConfig;

fn event_line(writer: impl io::Write, val: serde_json::Value) -> Result<()> {
    event_line_with_ending(writer, val, true)
}

fn event_line_with_ending(mut writer: impl io::Write, val: serde_json::Value, ending: bool) -> Result<()> {
    serde_json::to_writer(&mut writer, &val)?;
    if ending {
        writer.write_all(b",\n")?;
    }
    Ok(())
}

pub fn render_chrome_trace<'a, E>(events: E) -> Result<()>
where
    E: IntoIterator<Item = &'a (Event<SystemState>, SystemState)>,
{
    let path: PathBuf = config().get("output_file")?;
    let mut file = BufWriter::new(File::create(path)?);
    file.write_all(b"{\"traceEvents\":[\n")?;

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

    for (evt, _) in events.into_iter() {
        match evt.state() {
            SystemState::IncomingJobs { jobs } => {
                for job in jobs.iter() {
                    // queuing time are drawn as duration on thread 0
                    event_line(
                        &mut file,
                        json!({
                            "name": format!("Job {}", job.id),
                            "ph": "B",
                            "cat": "queuing",
                            "ts": evt.time(),
                            "id": job.id,
                            "tid": job.id,
                            "pid": 0,
                            "args": {
                                "job_id": job.id,
                                "job_len": job.length,
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
                                "ts": evt.time(),
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
                                "ts": evt.time() + budget,
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
                                "ts": evt.time() + budget,
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
            SystemState::BatchDone(batch) => {
                // the whole batch span
                event_line(
                    &mut file,
                    json!({
                        "name": "Batch",
                        "ph": "X",
                        "cat": "exec.batch",
                        "ts": batch.started,
                        "dur": evt.time() - batch.started,
                        "tid": 0,
                        "pid": 1,
                        "args": {
                            "batch_size": batch.jobs.len(),
                        }
                    }),
                )?;
                for (idx, job) in batch.jobs.iter().enumerate() {
                    let idx = idx + 1;
                    // flow arrow start
                    event_line(
                        &mut file,
                        json!({
                            "name": format!("Job {}", job.id),
                            "ph": "s",
                            "cat": "scheduling",
                            "ts": batch.started,
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
                            "ts": batch.started,
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
                            "ts": batch.started + 0.01,
                            "id": job.id,
                            "tid": idx,
                            "pid": 1,
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
                            "ts": batch.started,
                            "dur": job.length,
                            "id": job.id,
                            "tid": idx,
                            "pid": 1,
                            "args": {
                                "job_id": job.id,
                            }
                        }),
                    )?;
                }
            }
            SystemState::JobsPastDue(jobs) => {
                for job in jobs.iter() {
                    // finish the queuing span
                    event_line(
                        &mut file,
                        json!({
                            "name": format!("Job {}", job.id),
                            "ph": "E",
                            "cat": "queuing",
                            "ts": evt.time(),
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
                past_due += jobs.len();
                event_line(
                    &mut file,
                    json!({
                        "name": "Past Due Jobs",
                        "ph": "C",
                        "cat": "past_due",
                        "ts": evt.time(),
                        "pid": 0,
                        "args": {
                            "past_due": past_due,
                        }
                    }),
                )?;
            }
            _ => (),
        }
    }

    event_line(
        &mut file,
        json!({
            "name": "process_name",
            "ph": "M",
            "pid": 0,
            "args": {
                "name": "Pending Jobs"
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
            "pid": 1,
            "args": {
                "sort_index": 1
            }
        }),
    )?;
    event_line_with_ending(
        &mut file,
        json!({
            "name": "process_name",
            "ph": "M",
            "pid": 1,
            "args": {
                "name": "Batch 0"
            }
        }),
        false,
    )?;
    file.write_all(b"\n],\"config\":")?;

    let cfg: SimConfig = config().fetch()?;
    serde_json::to_writer(&mut file, &cfg)?;
    file.write_all(b"\n}")?;
    Ok(())
}
