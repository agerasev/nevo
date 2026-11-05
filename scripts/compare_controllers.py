#!/usr/bin/env python3
"""Run paired frozen-controller assays using the original long-run simulation.

Example:
  python3 scripts/compare_controllers.py --checkpoint runs/RUN/checkpoints/FILE.cbor \
      --output runs/controller-comparison --seeds 32 --ticks 20000

Uses the repository's b86d5bd source and locked, locally cached Rust dependencies.
All generated source, binaries, initial/final states, and results stay in --output.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import time

ROOT=Path(__file__).resolve().parents[1]
REVISION='b86d5bd'

def run(command, **kwargs):
    return subprocess.run(command, check=True, **kwargs)

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--checkpoint',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--seeds',type=int,default=32)
    p.add_argument('--ticks',type=int,default=20000)
    p.add_argument('--workers',type=int,default=4)
    args=p.parse_args()
    if args.seeds<1 or args.workers<1 or args.ticks<2 or args.ticks%2:
        p.error('positive seeds/workers and a positive even tick budget required')
    checkpoint=args.checkpoint.resolve(); out=args.output.resolve()
    if (out/'protocol.json').exists():
        p.error('output already has a protocol; choose a fresh directory')
    if not (ROOT/'wgame/wgame/Cargo.toml').exists():
        p.error('initialize the wgame submodule before building')
    out.mkdir(parents=True,exist_ok=True); source=out/'source';source.mkdir(exist_ok=True)
    archive=subprocess.check_output(['git','archive',REVISION,'Cargo.toml','Cargo.lock','src'],cwd=ROOT)
    with tarfile.open(fileobj=io.BytesIO(archive)) as bundle:
        bundle.extractall(source)
    if not (source/'wgame').exists():
        (source/'wgame').symlink_to(ROOT/'wgame',target_is_directory=True)
    (source/'src/bin').mkdir(exist_ok=True)
    harness=Path(__file__).with_suffix('.rs')
    shutil.copyfile(harness,source/'src/bin/compare_controllers.rs')
    run(['cargo','build','--offline','--locked','--release','--no-default-features',
         '--bin','compare_controllers','--manifest-path',str(source/'Cargo.toml'),
         '--target-dir',str(out/'build')],cwd=ROOT)
    executable=out/'build/release/compare_controllers'
    revision=subprocess.check_output(['git','rev-parse',REVISION],cwd=ROOT,text=True).strip()
    protocol=dict(simulation_revision=revision,checkpoint=str(checkpoint),checkpoint_sha256=digest(checkpoint),
                  harness_sha256=digest(harness),runner_sha256=digest(Path(__file__)),
                  rustc=subprocess.check_output(['rustc','--version'],text=True).strip(),
                  seeds=list(range(1,args.seeds+1)),ticks=args.ticks,workers=args.workers,
                  founder_energy=100,plants=500,initial_plant_energy=400,
                  founders='one of every saved genome; independently shuffled over matched positions',
                  recurrent_memory='reset to zero',mutation=0,immigration=0,reproduction=True,
                  gradient_rule='direction = observation.gradient; speed_fraction = 1; zero gradient means zero displacement',
                  endpoints=['extinction','mean population','food per tick','births/deaths'],
                  late_window=[args.ticks//2,args.ticks],
                  matching='identical initial serialized states and RNG streams; later states diverge with behavior')
    (out/'protocol.json').write_text(json.dumps(protocol,indent=2)+'\n')
    # A held-out seed tests repeatability; it is not included in the results.
    for label in ['verify-a','verify-b']:
        run([str(executable),str(checkpoint),'0','1000',str(out/label)])
    for name in ['initial.cbor','windows.jsonl','summary.json','evolved_rnn-final.cbor','gradient_max-final.cbor']:
        assert (out/'verify-a'/name).read_bytes()==(out/'verify-b'/name).read_bytes(), f'replay mismatch: {name}'
    (out/'verification.json').write_text(json.dumps({'seed':0,'ticks':1000,'exact_repeat_files':5},indent=2)+'\n')
    def trial(seed):
        trial_start=time.monotonic()
        result=run([str(executable),str(checkpoint),str(seed),str(args.ticks),str(out/f'seed-{seed:03d}')],capture_output=True,text=True)
        return f'{result.stdout.strip()} ({time.monotonic()-trial_start:.1f}s)'
    start=time.monotonic()
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        tasks=[pool.submit(trial,seed) for seed in protocol['seeds']]
        for task in as_completed(tasks):print(task.result(),flush=True)
    (out/'timing.json').write_text(json.dumps({'elapsed_seconds':time.monotonic()-start},indent=2)+'\n')
    print(f'Completed {args.seeds} matched pairs in {time.monotonic()-start:.1f}s',flush=True)

if __name__=='__main__':main()
