import { useEffect,useLayoutEffect,useState } from 'preact/hooks';
import { Modal,useUi } from './ui.tsx';
import type { PhysicalRow } from './model.ts';
import { FeedbackScope,feedbackWritable,type FeedbackValues } from './feedback.ts';

export function Feedback({row,close}:{row:PhysicalRow;close:()=>void}){
  const ui=useUi(),[scope]=useState(()=>new FeedbackScope(ui,row));
  const [values,setValues]=useState<FeedbackValues|null>(null),[loaded,setLoaded]=useState(false),[error,setError]=useState('');
  useLayoutEffect(()=>()=>{scope.cancelled=true;},[scope]);
  const dismiss=()=>{scope.cancelled=true;close();};
  const load=()=>void ui.run(async()=>{setError('');setLoaded(false);try{const v=await scope.read();setValues(v);setLoaded(true);}catch(e){if(!scope.cancelled)setError((e as Error).message);throw e;}});
  useEffect(()=>{load();},[]);
  const toggle=()=>void ui.run(async()=>{try{setError('');setValues(await scope.set(values?.active!==true));}catch(e){if(!scope.cancelled)setError((e as Error).message);throw e;}});
  const capability=values?.capability??0;
  return <Modal title={`Feedback · ${row.item.name}`} close={dismiss}>
    <p class="hint">{scope.initial.inventory.name}/{row.bus.name} · address {row.kind==='input'?row.item.short_address:row.parent?.short_address}{row.kind==='instance'?` / instance ${row.item.number}`:' · module'}</p>
    {error&&<p class="error" role="alert">{error}</p>}
    {!loaded&&!error&&<p role="status">Reading feedback…</p>}
    {loaded&&(feedbackWritable(values)?<div class="toolbar"><span>{(capability&1)!==0?'Visible indicator':''}{(capability&9)===9?' and ':''}{(capability&8)!==0?'Audible feedback':''}</span><span class="spacer"/><button class={`power ${values?.active?'on':'off'}`} aria-label="Feedback active" aria-pressed={values?.active??false} disabled={!!ui.busy} onClick={toggle}>{values?.active?'On':'Off'}</button></div>:<p>{values?.supported===false?'This input does not report supported feedback.':'Feedback is unavailable. Refresh to read it again.'}</p>)}
    <div class="dialog-actions"><button onClick={dismiss}>Close</button><button disabled={!!ui.busy} onClick={load}>Refresh feedback</button></div>
  </Modal>;
}
