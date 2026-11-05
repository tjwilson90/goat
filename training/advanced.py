"""Paired, seat-rotated evaluation of the selected rummy search."""
import argparse
import hashlib
import json
from pathlib import Path
import numpy as np
import torch
from train import engine, load, tournament

VARIANTS = {
    'prior': {},
    'selected': {'advanced': json.loads(Path(__file__).with_name('configs').joinpath('one-deck-advanced.json').read_text())},
}

def write(path, data):
    path=Path(path); path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data,indent=2)+'\n')

def evaluate(args):
    model,_=load(args.model)
    native=engine.NativeModel(str(Path(args.model).with_suffix('.json')))
    variants={k:VARIANTS[k] for k in args.variants}
    if args.configs:
        variants.update(json.loads(Path(args.configs).read_text()))
    output={'seed':args.seed,'seeds':args.seeds,'model':args.model,'variants':variants,'results':[]}
    output['model_sha256']=hashlib.sha256(Path(args.model).read_bytes()).hexdigest()
    output['engine_sha256']={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in Path(engine.__file__).parent.glob('*.so')}
    output['source_sha256']={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in
        [Path(__file__),Path('training/train.py'),Path('crates/goat_bot/src/rummy_advanced.rs'),Path('crates/goat_api/src/rummy_hand.rs')]}
    for name, options in variants.items():
        for opponent in args.opponents:
            for players in args.players:
                row=tournament(model,seeds=range(args.seed,args.seed+args.seeds),players=players,decks=1,
                    opponent=opponent,native=native,batch_size=args.batch,**options)
                row['variant']=name
                output['results'].append(row); write(args.output,output)
                print(json.dumps({k:v for k,v in row.items() if k!='cluster_losses'}),flush=True)
    summaries={}
    for name in variants:
        rows=[r for r in output['results'] if r['variant']==name]
        summaries[name]=float(np.mean([r['players']*r['goat_rate'] for r in rows]))
    output['normalized_scores']=summaries
    # Paired uncertainty clusters all seat rotations for a given deal.
    output['paired_vs_first']=[]
    first=next(iter(variants))
    for row in output['results']:
        if row['variant']==first: continue
        base=next(r for r in output['results'] if r['variant']==first and r['players']==row['players'] and r['opponent']==row['opponent'])
        diff=np.array(row['cluster_losses'])-base['cluster_losses']
        half=1.96*diff.std(ddof=1)/np.sqrt(len(diff)) if len(diff)>1 else float('nan')
        output['paired_vs_first'].append({'variant':row['variant'],'opponent':row['opponent'],'players':row['players'],
            'difference':float(diff.mean()),'ci95':[float(diff.mean()-half),float(diff.mean()+half)]})
    output['aggregate_paired_vs_first']={}
    for name in variants:
        if name==first: continue
        differences=[]
        for row in output['results']:
            if row['variant']!=name: continue
            base=next(r for r in output['results'] if r['variant']==first and r['players']==row['players'] and r['opponent']==row['opponent'])
            differences.append(row['players']*(np.array(row['cluster_losses'])-base['cluster_losses']))
        diff=np.mean(differences,axis=0)
        half=1.96*diff.std(ddof=1)/np.sqrt(len(diff)) if len(diff)>1 else float('nan')
        output['aggregate_paired_vs_first'][name]={'difference':float(diff.mean()),'ci95':[float(diff.mean()-half),float(diff.mean()+half)]}
    write(args.output,output); print(json.dumps(summaries),flush=True)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__); sub=parser.add_subparsers(dest='command',required=True)
    common=argparse.ArgumentParser(add_help=False)
    common.add_argument('--players',nargs='+',type=int,default=[3,4,5,6]); common.add_argument('--opponents',nargs='+',default=['simple','simulate-128','mixed'])
    common.add_argument('--seeds',type=int,default=40); common.add_argument('--seed',type=int,default=9000000); common.add_argument('--output',required=True)
    p=sub.add_parser('evaluate',parents=[common]); p.add_argument('--model',default='training/runs/one-deck-v2/best.pt')
    p.add_argument('--variants',nargs='*',choices=list(VARIANTS),default=['prior','selected']); p.add_argument('--configs'); p.add_argument('--batch',type=int,default=128)
    args=parser.parse_args(); torch.set_num_threads(2)
    evaluate(args)
