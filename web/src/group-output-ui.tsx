import {useLayoutEffect,useRef,useState} from 'preact/hooks';
import {useUi} from './ui.tsx';
import {busReady,type PhysicalRow} from './model.ts';
import {GroupOutputScope,type GroupResult} from './group-output.ts';

export function GroupPower({row}:{row:PhysicalRow}){
  const ui=useUi(),scope=useRef<GroupOutputScope|null>(null),[result,setResult]=useState<GroupResult|null>(null),[level,setLevel]=useState(50);
  useLayoutEffect(()=>()=>{if(scope.current)scope.current.cancelled=true;},[]);
  const operate=(patch?:Record<string,unknown>)=>void ui.run(async()=>{
    if(scope.current)scope.current.cancelled=true;ui.cancelled=false;const selected=new GroupOutputScope(ui,row);scope.current=selected;setResult(null);
    const actual=patch?await selected.set(patch):await selected.read();setResult(actual);
  });
  const actual=scope.current?.current()?result:null,v=actual?.values,ready=busReady(ui.snapshot,row.bus)&&ui.client.views.ready&&!ui.busy;
  const power=typeof v?.power_state==='boolean'?v.power_state:null;
  return <section class="card" aria-label="Group power and dimming">
    <div class="toolbar"><strong>Power</strong>{power===null?<><span class="muted">{v?.power_mixed?'Mixed':'Unknown'}</span><button aria-label="Group on" disabled={!ready} onClick={()=>operate({power:true})}>On</button><button aria-label="Group off" disabled={!ready} onClick={()=>operate({power:false})}>Off</button></>:<button class={`power ${power?'on':'off'}`} aria-label="Group power" aria-pressed={power} disabled={!ready} onClick={()=>operate({power:!power})}>{power?'On':'Off'}</button>}<span class="spacer"/><button disabled={!ready} onClick={()=>operate()}>Read group values</button></div>
    <div class="toolbar"><span>Brightness: {typeof v?.brightness==='number'?`${Math.round(v.brightness*100)}%`:v?.brightness_mixed?'Mixed':'Unknown'}</span><label>New level <input type="range" aria-label="Group level command" min="0" max="100" step="1" value={level} disabled={!ready} onInput={e=>setLevel(Number(e.currentTarget.value))}/></label><span>{level}%</span><button disabled={!ready} onClick={()=>operate({brightness:level/100})}>Set group level</button></div>
    <p class="hint">Commands use hardware group {row.item.group_address}. Readings cover saved fixtures.{actual?.membership_matches===false?' Assignments differ; use Verify all groups to review.':''}{v&&['partial','unavailable'].includes(v.quality)?' Some fixtures could not be read.':''}{v?.quality==='empty'?' No saved fixture currently reports membership.':''}</p>
  </section>;
}
