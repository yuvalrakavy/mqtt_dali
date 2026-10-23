import { useEffect, useLayoutEffect, useState } from 'preact/hooks';
import { Modal, useUi } from './ui.tsx';
import { type PhysicalRow } from './model.ts';
import { InputSettingsScope, changedSettings, eventFlags, type InputValues } from './input-settings.ts';
import {timerKeys,timerDisplay,timerRaw,type TimerKey} from './button-timing.ts';
import {applicableSensors,sensorDisplay,sensorRaw,type SensorKey} from './sensor-settings.ts';

export function InputSettings({row,close}:{row:PhysicalRow;close:()=>void}){
  const ui=useUi(),[scope]=useState(()=>new InputSettingsScope(ui,row));
  const [draft,setDraft]=useState<InputValues|null>(null),[error,setError]=useState('');
  const [timerText,setTimerText]=useState<Partial<Record<TimerKey,string>>>({}),[sensorText,setSensorText]=useState<Partial<Record<SensorKey,string>>>({});
  useLayoutEffect(()=>()=>{scope.cancelled=true;},[scope]);
  const dismiss=()=>{scope.cancelled=true;close();};
  const load=()=>void ui.run(async()=>{setError('');setDraft(null);setTimerText({});setSensorText({});try{const values=await scope.read();setDraft(values);}catch(e){if(!scope.cancelled)setError((e as Error).message);throw e;}});
  useEffect(()=>{load();},[]);
  const update=(patch:Partial<InputValues>|((current:InputValues)=>Partial<InputValues>))=>setDraft(current=>current?{...current,...(typeof patch==='function'?patch(current):patch)}:null);
  const updateTimer=(key:TimerKey,text:string)=>{
    setTimerText(current=>({...current,[key]:text}));
    update(current=>({button_timers:current.button_timers?{...current.button_timers,[key]:timerRaw(key,text)}:current.button_timers}));
  };
  const updateSensor=(key:SensorKey,text:string)=>{
    setSensorText(current=>({...current,[key]:text}));
    update(current=>({sensor_settings:current.sensor_settings?{...current.sensor_settings,[key]:sensorRaw(key,text)}:current.sensor_settings}));
  };
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
      <fieldset><legend>Events to send</legend>{flags?flags.map((label,bit)=><label><input type="checkbox" checked={!!(draft.event_filter&(1<<bit))} onChange={e=>{const checked=e.currentTarget.checked;update(current=>({event_filter:checked?current.event_filter|(1<<bit):current.event_filter&~(1<<bit)}));}}/>{label}</label>):<p>Raw filter: {draft.event_filter.toString(16).toUpperCase().padStart(6,'0')}. This instance type has no known common event labels.</p>}</fieldset>
      <fieldset><legend>Input instance groups</legend>{['Primary group','Instance group 1','Instance group 2'].map((label,index)=><label>{label}<select aria-label={label} value={draft.groups[index]} onChange={e=>{const value=Number(e.currentTarget.value);update(current=>{const groups=[...current.groups];groups[index]=value;return {groups};});}}><option value="255">Unassigned</option>{Array.from({length:32},(_,n)=><option value={n}>{n}</option>)}</select></label>)}</fieldset>
      {row.item.instance_type===1&&<fieldset><legend>Button timing</legend>{scope.values?.button_timers?.quality==='complete'&&draft.button_timers?<>
        <div class="input-timer-grid">{timerKeys.map(key=>{
          const timers=draft.button_timers!,minimum=key==='short'?timers.short_min!*20:key==='double'?0:key==='repeat'?100:5;
          const label=({short:'Short press (ms)',double:'Double press (ms)',repeat:'Long repeat (ms)',stuck:'Stuck button (s)'})[key],raw=timers[key]!;
          return <label>{label}<input type="number" inputMode="numeric" aria-label={label} min={minimum} max={key==='short'?5100:key==='stuck'?255:2000} step={key==='stuck'?1:20} value={timerText[key]??(Number.isFinite(raw)?timerDisplay(key,raw):'')} onInput={e=>updateTimer(key,e.currentTarget.value)}/></label>;
        })}</div><p class="hint">Short minimum: {draft.button_timers.short_min!*20}ms. Double minimum: {draft.button_timers.double_min!*20}ms; 0 disables double press. Other millisecond values use 20ms steps.</p>
      </>:<p class="hint">Button timing is unavailable or incomplete. Basic event settings remain available.</p>}</fieldset>}
      {applicableSensors(row.item.instance_type??-1).length>0&&<fieldset><legend>Sensor settings</legend>{scope.values?.sensor_settings?.quality==='complete'&&draft.sensor_settings?<>
        <div class="input-timer-grid">{applicableSensors(row.item.instance_type!).map(key=>{
          const s=draft.sensor_settings!;
          if(key==='hold'&&!s.hold_supported)return <p class="hint">Hold time is not supported by this sensor.</p>;
          const label=({report:'Report interval (s)',deadtime:'Deadtime (ms)',hold:'Hold time (s)',hysteresis:'Hysteresis (%)',hysteresis_min:'Minimum hysteresis (sensor units)'})[key],raw=s[key]!;
          return <label>{label}<input type="number" inputMode="numeric" aria-label={label} min={key==='hold'?1:0} max={key==='deadtime'?12750:key==='hold'?2540:key==='hysteresis'?25:255} step={key==='deadtime'?50:1} value={sensorText[key]??(Number.isFinite(raw)?sensorDisplay(key,raw):'')} onInput={e=>updateSensor(key,e.currentTarget.value)}/></label>;
        })}</div><p class="hint">0 disables reporting, deadtime or percentage hysteresis.{row.item.instance_type===3?' Hold time: 1s or 10s increments.':''}{row.item.instance_type===4?' Minimum hysteresis uses sensor input units.':''}</p>
      </>:<p class="hint">Sensor settings are unavailable or incomplete. Basic event settings remain available.</p>}</fieldset>}
    </fieldset>}
    <div class="dialog-actions"><button onClick={dismiss}>Cancel</button><button disabled={!!ui.busy} onClick={load}>Refresh settings</button><button class="primary" disabled={!!ui.busy||!draft} onClick={save}>Save settings</button></div>
  </Modal>;
}
