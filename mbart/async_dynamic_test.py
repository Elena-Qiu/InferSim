from __future__ import print_function

import sys
import time
import traceback
import json
import csv


def get_full_class_name(obj):
    module = obj.__class__.__module__
    if module is None or module == str.__class__.__module__:  # type: ignore
        return obj.__class__.__name__
    return module + '.' + obj.__class__.__name__


async def setup_clipper(args):
    '''Setup the clipper cluster, returns the clipper connection and the endpoint url'''
    import asyncio
    import aiohttp
    from clipper_admin import ClipperConnection, DockerContainerManager
    from clipper_admin.exceptions import ClipperException
    from clipper_admin.deployers import python as python_deployer

    clipper_conn = ClipperConnection(DockerContainerManager(use_centralized_log=False))

    try:
        # start or connect to the cluster
        try:
            # this blocks until the cluster is ready
            clipper_conn.start_clipper()
        except ClipperException:
            clipper_conn.connect()

        # deploy the model and register the application
        # this blocks until the model is ready
        name = 'translation-model'
        input_type = "strings"
        filename = args.reqs
        with open(filename) as f:
            reader = csv.DictReader(f)
            first_row = next(reader)
            deadline = int(float(first_row['Deadline']) - float(first_row['Admitted']))*1000
        clipper_conn.register_application(name, input_type, "None",
                                        slo_micros=deadline)
        clipper_conn.deploy_model(name=name, version=1, input_type=input_type,
                        image="mbart")
        clipper_conn.link_model_to_app(name, name)

        # wait a few second for the model container to stablize
        await asyncio.sleep(20)

        retry = 3
        # wait for replicas to spin up for 3s
        while retry > 0:
            if clipper_conn.get_num_replicas(name) > 0:
                break
            print('INFO: waiting for replicas to spin up', file=sys.stderr)
            await asyncio.sleep(1)
            retry -= 1
        else:
            # something wrong
            print('ERROR: replicas take too long to spin up, possibly died. Check container log', file=sys.stderr)
            raise TypeError('Bad python model')

        # endpoint url
        clipper_conn.get_query_addr()
        endpoint = f"http://{clipper_conn.get_query_addr()}/translation-model/predict"

        # wait for container to be ready
        async with aiohttp.ClientSession() as http_client:
            retry = 10
            while retry > 0:
                try:
                    await predict(http_client, endpoint, "Hello, nice to meet you!")
                    break
                except:
                    print('INFO: waiting for ready to serve', file=sys.stderr)
                    await asyncio.sleep(1)
                    retry -= 1
            else:
                # something wrong
                print('ERROR: replicas take too long to spin up, possibly died. Check container log', file=sys.stderr)
                raise TypeError('Bad python model')

        print('INFO: ready to go', file=sys.stderr)

        return clipper_conn, endpoint
    except Exception as e:
        # cleanup if error
        print('ERROR: error when starting clipper, clean up', file=sys.stderr)
        clipper_conn.stop_all()
        raise e


async def predict(http_client, endpoint, input_sen):
    async with http_client.post(endpoint, json={'input': input_sen}) as r:
        r = await r.json()
        print(r)
        if r['output'] is None:
            return None
        else:
            return r['output']


async def fetch(now_ms, input_sen, http_client, endpoint, args):
    try:
        if input_sen is not None:
            print(f'INFO: at {now_ms:.3f} ms fetching a sentence', file=sys.stderr)
        else:
            print(f'INFO: at {now_ms:.3f} ms fetching None ms', file=sys.stderr)

        started = time.perf_counter()
        output_sen = await predict(http_client, endpoint, input_sen)
        # measured latency
        latency_us = (time.perf_counter() - started) * 1000
        if output_sen is "None":
            args.print(f'{now_ms}::::past_due:')
        else:
            args.print(f'{now_ms}:{input_sen[:5]}:{latency_us}:{output_sen[:4]}:done:')
    except Exception as e:
        ename = get_full_class_name(e)
        args.print(f'{now_ms}::::error:{ename}')
        if args.debug:
            print('Error: ', traceback.format_exc(), file=sys.stderr)
            raise e


def incoming_file(filename: str):
    """read delay and length from csv file
    yields (delay_ms, length_ms)
    """
    with open(filename) as f:
        reader = csv.DictReader(f)
        jobs = [
            (float(row['Admitted']), str(row['InputSen']))
            for row in reader
        ]
    # jobs has to be sort by admitted
    jobs.sort()
    # take note of current time
    now = 0
    batch = []
    for admitted, input_sen in jobs:
        delay_ms = admitted - now
        if delay_ms > 0:
            yield batch, delay_ms
            now = admitted
            batch = []
        batch.append(input_sen)
    yield batch, delay_ms


async def queryer(endpoint, args):
    import aiohttp
    import asyncio

    # csv header
    args.print('Timestamp:InputSen:LatencyMS:OutputSen:State:EName')

    async with aiohttp.ClientSession() as http_client:
        # start fetching
        incoming = incoming_file(args.reqs)
        flying = []
        base_ms = time.perf_counter() * 1000
        get_time = lambda: time.perf_counter() * 1000 - base_ms
        print('INFO: rock and roll', file=sys.stderr)
        count = 0
        for input_sens, delay_ms in incoming:
            count += 1
            now_ms = get_time()

            # lengths_str = ', '.join([l for l in input_sens])
            print(f'INFO: at {now_ms:.3f} ms batch and delay {delay_ms:.3f} ms', file=sys.stderr)

            # fire current request
            if input_sens:
                flying.extend(
                    asyncio.create_task(fetch(now_ms, input_sen, http_client, endpoint, args))
                    for input_sen in input_sens
                )

            remaining_ms = delay_ms
            try:
                # use remaining time to do some book keeping
                remaining_ms = delay_ms - (get_time() - now_ms)
                if remaining_ms > 0 and flying:
                    # book keeping
                    done, pending = await asyncio.wait(flying, timeout=0)
                    flying = list(pending)
                    # re-raise any exception if debug 
                    if args.debug:
                        for r in done:
                            r.result()

                remaining_ms = delay_ms - (get_time() - now_ms)
                # wait until delay_ms
                if remaining_ms > 0:
                    await asyncio.sleep(remaining_ms / 1000)
                    remaining_ms = delay_ms - (get_time() - now_ms)
                else:
                    if remaining_ms < -5:
                        print(f'WARNING: bookkeeping for too long: {remaining_ms}ms', file=sys.stderr)
                        continue
                if remaining_ms < -5:
                    print(f'WARNING: slept for too long: {remaining_ms}ms', file=sys.stderr)
                    continue
            finally:
                pass
        print('INFO: done', file=sys.stderr)


async def amain():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--debug", action="store_true", help="Show response error", default=False)
    parser.add_argument("--output", type=str, help="Output file", default="output.csv")
    parser.add_argument("--reqs", type=str, help="Request schedule csv file", default="req.csv")

    args = parser.parse_args()
    with open(args.output, 'w') as f:
        def printer(*args, **kwargs):
            print(*args, **{'file': f, **kwargs})
            f.flush()

        # printer('# ' + json.dumps(vars(args)))
        args.print = printer

        clipper_conn, endpoint = await setup_clipper(args)
        try:
            await queryer(endpoint, args)
        finally:
            print('INFO: stop clipper')
            clipper_conn.stop_all()


def main():
    import asyncio
    asyncio.run(amain())


if __name__ == '__main__':
    main()