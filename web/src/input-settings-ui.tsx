import { useEffect, useLayoutEffect, useState } from 'preact/hooks';
import { Modal, useUi } from './ui.tsx';
import { type PhysicalRow } from './model.ts';
import { InputSettingsScope, changedSettings, eventFlags, type InputValues } from './input-settings.ts';

export function InputSettings({row,close}:{row:PhysicalRow;close:()=>void}){
  const ui=useUi(),[scope]=useState(()=>new InputSettingsScope(ui,row));
  const [draft,setDraft]=useState<InputValues|null>(null),[error,setError]=useState('');
  useLayoutEffect(()=>()=>{scope.cancelled=true;},[scope]);
  const dismiss=()=>{scope.cancelled=true;close();};
  const load=()=>void ui.run(async()=>{setError('');setDraft(null);try{const values=await scope.read();setDraft(values);}catch(e){if(!scope.cancelled)setError((e as Error).message);throw e;}});
  useEffect(()=>{load();},[]);
  const update=(patch:Partial<InputValues>)=>setDraft(draft?{...draft,...patch}:null);
  const save=()=>void ui.run(async()=>{try{if(!draft||!scope.values)return;await scope.save(changedSettings(scope.values,draft));dismiss();}catch(e){if(!scope.cancelled)setError((e as Error).message);throw e;}});
  const flags=eventFlags[row.item.instance_type??-1];
  return <Modal title={`Event settings · ${row.item.name}`} close={dismiss}>
    <p class="hint">{scope.initial.inventory.name}/{row.bus.name} · address {row.parent?.short_address} / instance {row.item.number}. Changes are applied to this input's hardware.</p>
    {error&&<p class="error" role="alert">{error}</p>}
    {!draft&&!error&&<p role="status">Reading input settings…</p>}
    {draft&&<fieldset class="input-settings-form" disabled={!!ui.busy}>
      <label><input type="checkbox" checked={draft.enabled} onChange={e=>update({enabled:e.currentTarget.checked})}/> Input enabled</label>
      <label>Event priority<select aria-label="Event priority" value={draft.event_priority} onChange={e=>update({event_priority:Number(e.currentTarget.value)})}>{[2,3,4,5].map(v=><option value={v}>{v}{v===2?' · highest':v===5?' · lowest':''}</option>)}</select></label>
      <label>Event addressing<select aria-label="Event addressing" value={draft.event_scheme} onChange={e=>update({event_scheme:Number(e.currentTarget.value)})}>{['Instance and type','Device and type','Device and instance','Device group and type','Instance group and type'].map((label,v)=><option value={v}>{label}</option>)}</select></label>
      <p class="hint">{draft.event_scheme===2?'Device and instance identifies the source uniquely.':draft.event_scheme===1?'Device and type is routed only when exactly one matching instance is saved.':'This scheme is currently diagnostic only in the bridge. Use Device and instance for routed events.'}</p>
      <fieldset><legend>Events to send</legend>{flags?flags.map((label,bit)=><label><input type="checkbox" checked={!!(draft.event_filter&(1<<bit))} onChange={e=>update({event_filter:e.currentTarget.checked?draft.event_filter|(1<<bit):draft.event_filter&~(1<<bit)})}/>{label}</label>):<p>Raw filter: {draft.event_filter.toString(16).toUpperCase().padStart(6,'0')}. This instance type has no known common event labels.</p>}</fieldset>
      <fieldset><legend>Input instance groups</legend>{['Primary group','Instance group 1','Instance group 2'].map((label,index)=><label>{label}<select aria-label={label} value={draft.groups[index]} onChange={e=>{const groups=[...draft.groups];groups[index]=Number(e.currentTarget.value);update({groups});}}><option value="255">Unassigned</option>{Array.from({length:32},(_,n)=><option value={n}>{n}</option>)}</select></label>)}</fieldset>
    </fieldset>}
    <div class="dialog-actions"><button onClick={dismiss}>Cancel</button><button disabled={!!ui.busy} onClick={load}>Refresh settings</button><button class="primary" disabled={!!ui.busy||!draft} onClick={save}>Save settings</button></div>
  </Modal>;
}
