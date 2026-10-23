import {useEffect,useLayoutEffect,useState} from 'preact/hooks';
import {Modal,useUi} from './ui.tsx';
import type {PhysicalRow} from './model.ts';
import {EmergencyScope,emergencySettings,emergencyWritable,emergencyValue,settingValue,validateEmergencyPatch,type EmergencyPatch,type EmergencyValues} from './emergency.ts';

const title=(text:string)=>text.replace(/_/g,' ');
const active=(flags:Record<string,boolean|null>)=>Object.values(flags).every(value=>value===null)?'Unavailable':Object.entries(flags).filter(([,value])=>value===true).map(([name])=>title(name)).join(', ')||'None reported';
function display(values:EmergencyValues,key:string){const field=values.fields[key];return field.status!=='value'?title(field.status):`${field.at_least?'At least ':''}${field.value}${field.unit==='raw'?'':` ${field.unit}`}`;}
export function EmergencyLighting({row,close}:{row:PhysicalRow;close:()=>void}){
  const ui=useUi(),[scope]=useState(()=>new EmergencyScope(ui,row)),[result,setResult]=useState<any>(null),[draft,setDraft]=useState<Record<string,string>>({}),[error,setError]=useState(''),[settingsOpen,setSettingsOpen]=useState(false);
  useLayoutEffect(()=>()=>{scope.cancelled=true;},[scope]);
  const dismiss=()=>{scope.cancelled=true;close();};
  const load=()=>void ui.run(async()=>{setError('');setResult(null);try{setResult(await scope.read());}catch(error){if(!scope.cancelled)setError((error as Error).message);throw error;}});
  useEffect(()=>{load();},[]);
  const save=()=>{
    const patch:EmergencyPatch={};
    try{
      for(const [key,text]of Object.entries(draft)){if(!text.trim())throw new Error('Enter numeric emergency settings.');patch[key]=Number(text);}
      for(const [interval,delay]of [['function_test_interval_days','function_delay_minutes'],['duration_test_interval_weeks','duration_delay_minutes']]){
        if(patch[delay]!==undefined&&patch[interval]===undefined)patch[interval]=settingValue(scope.values,interval)??NaN;
        if(patch[interval]===0)delete patch[delay];
        else if(patch[interval]!==undefined&&patch[delay]===undefined)patch[delay]=settingValue(scope.values,delay)??NaN;
      }
      validateEmergencyPatch(patch,scope.values);
    }catch(error){setError((error as Error).message);return;}
    void ui.run(async()=>{setError('');setResult(null);try{const r=await scope.save(patch);setResult(r);setDraft({});ui.notice='Emergency settings verified in hardware.';}catch(error){if(!scope.cancelled)setError((error as Error).message+' Your unsaved settings are retained.');throw error;}});
  };
  const test=(kind:'function'|'duration'|null)=>void ui.run(async()=>{setError('');setResult(null);try{const r=await scope.test(kind);setResult(r);ui.notice=kind===null?'Stop request sent. The current native status is shown.':'Test request sent. Refresh status to inspect progress and native results.';}catch(error){if(!scope.cancelled)setError((error as Error).message);throw error;}});
  let current=true;try{scope.assert();}catch{current=false;}
  const values=result?.observed as EmergencyValues|null|undefined,battery=values?emergencyValue(values,'battery_charge'):null;
  const controls=current&&!!scope.values&&['mode_raw','features_raw','status_raw'].every(key=>emergencyValue(scope.values,key)!==null)&&!scope.identifying();
  return <Modal title={`Emergency lighting · ${row.item.name}`} close={dismiss}>
    <p class="hint">{scope.initial.inventory.name}/{row.bus.name} · address {row.item.short_address}</p>
    {error&&<p class="error" role="alert">{error}</p>}
    {!current&&<p class="error">Configuration or connection changed. Reopen this dialog.</p>}
    {!result&&!error&&<p role="status">Reading emergency status…</p>}
    {result&&!values&&<p role="status">{result.support===false?'This fixture does not report DT1 emergency support.':'Emergency support or fixture status is unavailable.'}</p>}
    {values&&<section aria-label="Emergency status"><dl>
      <dt>Battery charge</dt><dd>{battery===null?'Unknown':`${(battery*100/254).toFixed(1)}%`}</dd>
      <dt>Mode</dt><dd>{active(values.mode)}</dd><dt>Faults</dt><dd>{active(values.failures)}</dd>
      <dt>Test status</dt><dd>{active(values.status)}</dd>
      <dt>Last duration result</dt><dd>{values.status.duration_done_valid===true?display(values,'last_duration_seconds'):'Unavailable (no valid native result)'}</dd>
      <dt>Rated duration</dt><dd>{display(values,'rated_duration_seconds')}</dd>
    </dl><p class="hint">These are explicit native readings. A completed command confirms the request, not completion of the battery test.</p></section>}
    <div class="dialog-actions"><button disabled={!!ui.busy||!current} onClick={load}>Refresh status</button><button disabled={!!ui.busy||!controls} onClick={()=>test('function')}>Function test</button><button disabled={!!ui.busy||!controls} onClick={()=>test('duration')}>Duration test</button><button disabled={!!ui.busy||!controls} onClick={()=>test(null)}>Stop test</button></div>
    <p class="hint">A duration test uses the battery for the fixture's native test period. Refresh status to see pending, running and last valid results.</p>
    {scope.identifying()&&<p class="hint">Stop Blink before emergency operations.</p>}
    {values&&<details open={settingsOpen} onToggle={event=>setSettingsOpen(event.currentTarget.open)}><summary>Emergency settings</summary><fieldset class="input-settings-form" disabled={!!ui.busy||!current||scope.identifying()}><div class="input-timer-grid">{emergencySettings.map(field=><label key={field.key}>{field.label}<input aria-label={field.label} type="number" min={field.min} max={field.max} step={field.step} placeholder="Unknown" value={draft[field.key]??String(settingValue(values,field.key)??'')} disabled={!emergencyWritable(field.key,scope.values)} onInput={event=>setDraft(previous=>({...previous,[field.key]:event.currentTarget.value}))}/></label>)}</div><p class="hint">Timeout is 15 minutes or whole days (1440 minutes each). An interval of 0 disables automatic tests. Saving an interval restarts its first test using the displayed delay; changing a first delay also restarts that schedule.</p><p class="hint">Manufacturer level limits: {emergencyValue(values,'minimum_level')??'unknown'}–{emergencyValue(values,'maximum_level')??'unknown'}. Pending timeout flags a failure while leaving the test pending.</p><button class="primary" disabled={!Object.keys(draft).length} onClick={save}>Save emergency settings</button></fieldset></details>}
    {Object.keys(draft).length>0&&!values&&<p class="hint">Your unsaved settings are retained. Refresh status to inspect the fixture before saving again.</p>}
    {result&&<details><summary>Native emergency evidence</summary><pre>{JSON.stringify(result,null,2)}</pre></details>}
    <div class="dialog-actions"><button onClick={dismiss}>Close</button></div>
  </Modal>;
}
