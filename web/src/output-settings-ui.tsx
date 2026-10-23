import {useEffect,useLayoutEffect,useState} from 'preact/hooks';
import {Modal,useUi} from './ui.tsx';
import type {PhysicalRow} from './model.ts';
import {OutputSettingsScope,changedOutputSettings,validateOutputPatch,outputWritable,switchingWritable,switchingFields,levelPercent,percentLevel,fadeTimes,fadeRates,type OutputValues,type OutputField,type OutputResult,type SwitchingField} from './output-settings.ts';

export function OutputSettings({row,close}:{row:PhysicalRow;close:()=>void}){
  const ui=useUi(),[scope]=useState(()=>new OutputSettingsScope(ui,row));
  const [draft,setDraft]=useState<OutputValues|null>(null),[result,setResult]=useState<OutputResult|null>(null),[error,setError]=useState('');
  const [percentText,setPercentText]=useState<Partial<Record<OutputField,string>>>({}),[startupMode,setStartupMode]=useState<Partial<Record<OutputField,string>>>({});
  const [switchingText,setSwitchingText]=useState<Partial<Record<SwitchingField,string>>>({});
  useLayoutEffect(()=>()=>{scope.cancelled=true;},[scope]);
  const dismiss=()=>{scope.cancelled=true;close();};
  const load=()=>void ui.run(async()=>{
    setError('');setDraft(null);setResult(null);setPercentText({});setStartupMode({});setSwitchingText({});
    try{const r=await scope.read();setResult(r);setDraft(r.settings);if(!r.settings)setError('Fixture settings are unavailable. Refresh to inspect again.');}
    catch(e){if(!scope.cancelled)setError((e as Error).message);throw e;}
  });
  useEffect(()=>{load();},[]);
  const update=(field:OutputField,value:number)=>setDraft(current=>current?{...current,[field]:value}:null);
  const save=()=>{
    if(!draft||!scope.values)return;const patch=changedOutputSettings(scope.values,draft);
    try{validateOutputPatch(patch,scope.values);}catch(e){setError((e as Error).message);return;}
    void ui.run(async()=>{setError('');try{await scope.save(patch);dismiss();}catch(e){if(!scope.cancelled){setDraft(null);setResult(null);setError((e as Error).message);}throw e;}});
  };
  const percent=(field:OutputField,label:string)=><label>{label}<input type="number" inputMode="decimal" min="0" max="100" step="0.1" aria-label={label} value={percentText[field]??(draft?.[field]!==null&&Number.isFinite(draft?.[field])?levelPercent(draft![field]!):'')} placeholder="Unknown" disabled={!outputWritable(field,scope.values)} onInput={e=>{const text=e.currentTarget.value;setPercentText(current=>({...current,[field]:text}));update(field,percentLevel(text));if(field==='power_on_level'||field==='system_failure_level')setStartupMode(current=>({...current,[field]:'level'}));}}/></label>;
  const startup=(field:'power_on_level'|'system_failure_level',label:string,special:string)=>{
    const value=draft?.[field],mode=startupMode[field]??(value===null?'unknown':value===0?'off':value===255?'special':'level');
    return <fieldset><legend>{label}</legend><label>{label} mode<select aria-label={`${label} mode`} value={mode} disabled={!outputWritable(field,scope.values)} onChange={e=>{const selected=e.currentTarget.value;setStartupMode(current=>({...current,[field]:selected}));setPercentText(current=>{const next={...current};delete next[field];return next;});update(field,selected==='off'?0:selected==='special'?255:254);}}>
      {value===null&&<option value="unknown">Unknown</option>}<option value="off">{draft?.switching_support===true?'Virtual level 0':'Off'}</option><option value="special">{special}</option><option value="level">{draft?.switching_support===true?'Set virtual level':'Set level'}</option>
    </select></label>{mode==='level'&&percent(field,`${label} ${draft?.switching_support===true?'virtual level':'brightness'} (%)`)}</fieldset>;
  };
  const flags=['Fixture failure','Lamp failure','Lamp on','Limit error','Fade running','Reset state','Missing short address','Power failure'];
  const relay=draft?.switching_support===true,labels:Record<SwitchingField,string>={upward_on:'Upward on threshold',upward_off:'Upward off threshold',downward_on:'Downward on threshold',downward_off:'Downward off threshold'};
  const threshold=(field:SwitchingField)=>{
    const value=draft?.switching?.[field],disabled=value===255,min=field.startsWith('upward_')?1:0,label=labels[field];
    const updateThreshold=(raw:number)=>setDraft(current=>current?.switching?{...current,switching:{...current.switching,[field]:raw}}:current);
    return <fieldset><legend>{label}</legend><label>Native level<input type="number" inputMode="numeric" min={min} max="254" step="1" aria-label={label} value={switchingText[field]??(value!==null&&!disabled&&Number.isFinite(value)?String(value):'')} placeholder={disabled?'Disabled':'Unknown'} disabled={disabled||!switchingWritable(field,scope.values)} onInput={e=>{const text=e.currentTarget.value;setSwitchingText(current=>({...current,[field]:text}));const raw=text.trim()===''?NaN:Number(text);updateThreshold(Number.isInteger(raw)&&raw>=min&&raw<=254?raw:NaN);}}/></label>
      <label class="check-field"><input type="checkbox" aria-label={`Disable ${label.toLowerCase()}`} checked={disabled} disabled={!switchingWritable(field,scope.values)} onChange={e=>{const checked=e.currentTarget.checked,text=switchingText[field],raw=text===undefined||text.trim()===''?min:Number(text);updateThreshold(checked?255:Number.isInteger(raw)&&raw>=min&&raw<=254?raw:min);}}/> Disabled</label></fieldset>;
  };
  return <Modal title={`Settings · ${row.item.name}`} close={dismiss}>
    <p class="hint">{scope.initial.inventory.name}/{row.bus.name} · address {row.item.short_address}. Changes are saved to this fixture.</p>
    {error&&<p class="error" role="alert">{error}</p>}
    {!draft&&!error&&<p role="status">Reading fixture settings…</p>}
    {draft&&<fieldset class="input-settings-form" disabled={!!ui.busy}>
      <div class="input-timer-grid">{percent('minimum_level',relay?'Minimum virtual level (%)':'Minimum brightness (%)')}{percent('maximum_level',relay?'Maximum virtual level (%)':'Maximum brightness (%)')}</div>
      <p class="hint">Physical minimum: {draft.physical_minimum===null?'unknown':`${levelPercent(draft.physical_minimum)}%`}. {relay?'These limits control a virtual level. Changing limits can operate the relay contact.':'Limits can change a light that is already on. An off light stays off.'}</p>
      <div class="input-timer-grid">{startup('power_on_level','Power on','Last active level')}{startup('system_failure_level','Bus failure','Keep current level')}</div>
      <div class="input-timer-grid">
        <label>Fade time<select aria-label="Fade time" value={draft.fade_time??''} disabled={!outputWritable('fade_time',scope.values)} onChange={e=>update('fade_time',Number(e.currentTarget.value))}>{draft.fade_time===null&&<option value="">Unknown</option>}{fadeTimes.map(v=><option value={v.code}>{v.label}</option>)}</select></label>
        <label>Dimming rate<select aria-label="Dimming rate" value={draft.fade_rate??''} disabled={!outputWritable('fade_rate',scope.values)} onChange={e=>update('fade_rate',Number(e.currentTarget.value))}>{draft.fade_rate===null&&<option value="" disabled>{draft.fade_raw===null?'Unknown':'Unknown (reserved code 0)'}</option>}{fadeRates.map(v=><option value={v.code}>{v.label}</option>)}</select></label>
      </div>
      <p class="hint">Fade durations are nominal. Code 0 uses other supported fade settings and does not guarantee an instant transition. Dimming rate is used by up/down commands.</p>
      {relay&&draft.switching&&<fieldset><legend>Relay thresholds</legend><p class="hint">{draft.switching.adjustable_thresholds===true?'Adjustable thresholds':draft.switching.adjustable_thresholds===false?'Fixed thresholds':'Threshold support unknown'}. Native virtual levels control when the contact turns on or off. Disabled thresholds do not act.</p><div class="input-timer-grid switching-threshold-grid">{switchingFields.map(threshold)}</div>{draft.switching.quality==='partial'&&<p class="hint">Some relay readings are unavailable. Readable supported thresholds remain editable.</p>}</fieldset>}
      {draft.quality==='partial'&&<p class="hint">Some readings are unavailable. Settings with readable support remain available.</p>}
    </fieldset>}
    {result&&<details><summary>Fixture diagnostics</summary><p>{relay?`Actual contact: ${result.actual_level===254?'On':result.actual_level===0?'Off':'unknown'}`:`Actual brightness: ${result.actual_level===null?'unknown':`${levelPercent(result.actual_level)}%`}`}</p><p>Reported version byte: {result.version===null?'unknown':`0x${result.version.toString(16).toUpperCase().padStart(2,'0')}`}</p><p>Active status flags: {result.status===null?'unknown':flags.filter((_,bit)=>result.status!&(1<<bit)).join(', ')||'none'}</p>{relay&&draft?.switching&&<><p>Relay feature byte: {draft.switching.features_raw??'unknown'} · type byte: {draft.switching.gear_type_raw??'unknown'} · extended version: {draft.switching.extended_version_raw??'unknown'}</p><p>Raw switching status: {draft.switching.switch_status_raw??'unknown'} (threshold diagnostics; not contact position)</p></>}</details>}
    {scope.identifying()&&<p class="hint">Stop Blink before changing fixture settings.</p>}
    <div class="dialog-actions"><button onClick={dismiss}>Cancel</button><button disabled={!!ui.busy} onClick={load}>Refresh settings</button><button class="primary" disabled={!!ui.busy||!draft||scope.identifying()} onClick={save}>Save settings</button></div>
  </Modal>;
}
