#!/usr/bin/env python3
"""Summarize paired common-garden trials; optional Matplotlib figures."""
import argparse
import json
from pathlib import Path
import numpy as np

METRICS={
    'mean_population':lambda d:d['animal_ticks']/d['ticks'],
    'food_per_tick':lambda d:d['food_gained']/d['ticks'],
    'births_per_1000':lambda d:1000*d['births']/d['ticks'],
    'deaths_per_1000':lambda d:1000*d['deaths']/d['ticks'],
    'food_per_animal_tick':lambda d:d['food_gained']/d['active_animal_ticks'] if d['active_animal_ticks'] else np.nan,
    'distance_per_animal_tick':lambda d:d['distance']/d['active_animal_ticks'] if d['active_animal_ticks'] else np.nan,
    'energy_per_meal':lambda d:d['food_gained']/d['meals'] if d['meals'] else np.nan,
}
ARMS=['evolved_rnn','gradient_max']

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run',type=Path)
    parser.add_argument('--plot',action='store_true')
    args=parser.parse_args();root=args.run
    protocol=json.loads((root/'protocol.json').read_text())
    trials=[json.loads((root/f'seed-{seed:03d}/summary.json').read_text()) for seed in protocol['seeds']]
    assert all(t['ticks']==protocol['ticks'] and t['initial_worlds_equal'] for t in trials)
    assert all(t[arm]['total']['immigrants']==0 for t in trials for arm in ARMS)
    rng=np.random.default_rng(1152026)
    bootstrap=rng.integers(0,len(trials),size=(20000,len(trials)))
    aggregate={'trials':len(trials),'bootstrap_draws':20000,'bootstrap_seed':1152026,
        'uncertainty':'percentile bootstrap resampling paired world seeds; exploratory marginal intervals',
        'survived':{arm:sum(t[arm]['extinction_tick'] is None for t in trials) for arm in ARMS}}
    arrays={}
    for span in ['total','late']:
        aggregate[span]={}
        for name,fn in METRICS.items():
            a,b=[np.array([fn(t[arm][span]) for t in trials]) for arm in ARMS]
            d=a-b;arrays[span,name]=[a,b]
            aggregate[span][name]={'rnn_mean':float(a.mean()),'gradient_mean':float(b.mean()),
                'paired_difference':float(d.mean()),'paired_bootstrap_95':np.quantile(d[bootstrap].mean(axis=1),[.025,.975]).tolist(),
                'seeds_with_positive_difference':int((d>0).sum())}
    (root/'aggregate.json').write_text(json.dumps(aggregate,indent=2,allow_nan=False)+'\n')
    for name in ['mean_population','food_per_tick','births_per_1000','deaths_per_1000','distance_per_animal_tick']:
        print(name,aggregate['late'][name])
    if not args.plot:return
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    plt.rcParams.update({'font.family':'DejaVu Sans','font.size':10,'axes.spines.top':False,
        'axes.spines.right':False,'axes.grid':True,'grid.alpha':.18,'axes.titleweight':'bold'})
    colors=['#2463a6','#c66a20'];labels=['Evolved RNNs','Gradient at max speed']
    logs=[[json.loads(line) for line in (root/f'seed-{s:03d}/windows.jsonl').read_text().splitlines()] for s in protocol['seeds']]
    fig,axs=plt.subplots(2,2,figsize=(11.5,7.7),layout='constrained')
    for arm,color,label in zip(ARMS,colors,labels):
        series=[[row for row in trial if row['controller']==arm] for trial in logs]
        times=np.array([row['tick'] for row in series[0]])
        pop=np.array([[row['events']['animal_ticks']/row['events']['ticks'] for row in seq] for seq in series])
        food=np.array([[row['events']['food_gained']/row['events']['ticks'] for row in seq] for seq in series])
        # Re-bin ten 100-tick windows for display only.
        for values,ax in [(pop,axs[0,0]),(food,axs[0,1])]:
            chunks=[slice(i,min(i+10,len(times))) for i in range(0,len(times),10)]
            x=np.array([times[c].mean() for c in chunks])
            y=np.column_stack([values[:,c].mean(axis=1) for c in chunks])
            ax.plot(x,y.mean(axis=0),color=color,lw=2,label=label)
            lo,hi=np.quantile(y,[.1,.9],axis=0);ax.fill_between(x,lo,hi,color=color,alpha=.17)
    axs[0,0].axhline(25,color='#636363',ls=':',lw=1);axs[0,0].set(title='Both sustain about 25 animals',xlabel='Tick',ylabel='Mean population',ylim=(20,35))
    axs[0,1].axhline(50,color='#636363',ls=':',lw=1);axs[0,1].set(title='Both approach the food-production limit',xlabel='Tick',ylabel='Food energy gained per tick',ylim=(45,70))
    axs[0,0].legend(fontsize=8)
    for ax,name,title,ylabel in [
        (axs[1,0],'food_per_tick','Late food collection: a ~0.1% difference','Food energy / tick (zoomed scale)'),
        (axs[1,1],'deaths_per_1000','Late deaths: lower turnover with RNNs','Deaths per 1,000 ticks')]:
        a,b=arrays['late',name]
        for v1,v2 in zip(a,b):ax.plot([0,1],[v1,v2],color='#a4a9b2',alpha=.55,lw=.8,zorder=1)
        for x,values,color in [(0,a,colors[0]),(1,b,colors[1])]:
            ax.scatter(np.full(len(values),x),values,color=color,s=16,alpha=.65,zorder=2)
            ax.scatter([x],[values.mean()],marker='_',s=650,color='black',linewidths=3,zorder=3)
        ax.set(title=title,ylabel=ylabel,xticks=[0,1],xticklabels=labels,xlim=(-.3,1.3))
    fig.suptitle(f'Frozen controllers in {len(trials)} matched worlds · no immigration\n{protocol["ticks"]:,} ticks; lower panels use the final half of each trial',fontsize=15,fontweight='bold')
    fig.savefig(root/'comparison.png',dpi=170);fig.savefig(root/'comparison.svg')
    print('Wrote comparison.png and comparison.svg')

if __name__=='__main__':main()
