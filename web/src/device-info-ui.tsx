import {useEffect,useLayoutEffect,useState} from 'preact/hooks';
import {Modal,useUi} from './ui.tsx';
import type {PhysicalRow} from './model.ts';
import {DeviceInfoScope,memoryWindow,type MemoryReport,type MemoryReading} from './device-info.ts';
import {luminaireFields,luminaireDisplay} from './luminaire.ts';
import {energyDisplay} from './energy.ts';
const energyBanks={202:'Active input',203:'Apparent input',204:'Load-side active'} as const;
const hex=(v:number)=>v.toString(16).toUpperCase().padStart(2,'0');
const value=(r:MemoryReading|null)=>r===null?'Not read':r.kind==='value'?`0x${hex(r.raw)} (${r.raw})`:r.kind==='no_response'?'No response':r.kind==='receive_collision'?'Receive collision':'Transmit collision';
export function DeviceInfo({row,close}:{row:PhysicalRow;close:()=>void}){
  const ui=useUi(),[scope]=useState(()=>new DeviceInfoScope(ui,row));
  const [result,setResult]=useState<MemoryReport|null>(null),[error,setError]=useState(''),[bank,setBank]=useState('0'),[start,setStart]=useState('0'),[count,setCount]=useState('16');
  const [selectedEnergy,setSelectedEnergy]=useState('202');
  useLayoutEffect(()=>()=>{scope.cancelled=true;},[scope]);
  const dismiss=()=>{scope.cancelled=true;close();};
  const read=(kind:'identity'|'memory'|'luminaire'|'energy'='identity')=>void ui.run(async()=>{
    setResult(null);setError('');
    try{
      const number=(text:string)=>text.trim()===''?NaN:Number(text);
      const window=kind==='memory'?memoryWindow(number(bank),number(start),number(count)):undefined;
      const r=kind==='luminaire'?await scope.readLuminaire():kind==='energy'?await scope.readEnergy(number(selectedEnergy)):await scope.read(window);if(!scope.cancelled)setResult(r);
    }catch(e){if(!scope.cancelled)setError((e as Error).message);throw e;}
  });
  useEffect(()=>{read();},[]);
  const identity=result?.identity,version=(v:number[]|null|undefined)=>v?v.join('.'):'Unknown';
  return <Modal title={`Device information · ${row.item.name}`} close={dismiss}>
    <p class="hint">{scope.initial.inventory.name}/{row.bus.name} · {row.kind==='output'?'fixture':'input module'} address {row.item.short_address}. Captured readings; refresh to read again.</p>
    {error&&<p class="error" role="alert">{error}</p>}
    {!result&&!error&&<p role="status">{ui.busy?'Reading device information…':'Choose a read below.'}</p>}
    {result&&<section aria-label="Device information result"><p>Read quality: <strong>{result.quality}</strong></p>
      {result.reason&&<p class="hint">{result.reason}</p>}
      {result.operation==='read_identity'&&<><dl class="device-information">
        <dt>GTIN</dt><dd>{identity?.gtin??'Unknown'}</dd>
        <dt>Serial number</dt><dd>{identity?.serial_number??'Unknown'}{identity?.serial_number?` (${identity.serial_bits}-bit)`:''}</dd>
        <dt>Firmware version</dt><dd>{version(identity?.firmware_version)}</dd>
        <dt>Hardware version</dt><dd>{version(identity?.hardware_version)}</dd>
        <dt>Identity layout</dt><dd>{identity?.layout??'Unknown'}</dd>
        <dt>DALI version byte</dt><dd>{value(result.version)}</dd>
        <dt>Last memory bank</dt><dd>{identity?.last_memory_bank??'Unknown'}</dd>
      </dl>{!identity&&<p class="hint">Identity is unavailable from these readings.</p>}</>}
      {result.luminaire&&<section aria-label="Luminaire data"><p class="hint">Manufacturer-supplied luminaire data. These readings do not change saved names or bindings.</p>
        {result.luminaire.format_status==='supported'?<dl class="device-information">{luminaireFields.map(([key,label])=><><dt key={`${key}-label`}>{label}</dt><dd key={`${key}-value`} data-luminaire-field={key}>{luminaireDisplay(key,result.luminaire!.fields[key])}</dd></>)}</dl>:<p class="hint">{result.luminaire.format_status==='unrecognized'?`Unrecognized luminaire data format 0x${result.luminaire.format_id!.toString(16).toUpperCase().padStart(4,'0')}. Raw bytes are available below.`:'Luminaire data is unavailable from these readings.'}</p>}
      </section>}
      {result.energy&&<section aria-label="Energy data"><p class="hint">{energyBanks[result.bank as keyof typeof energyBanks]} · Captured energy and power.</p>
        {result.energy.format_status==='supported'?<><p class="hint">Snapshot confirmed and released.</p><dl class="device-information">{(['energy','power'] as const).map(key=><><dt key={`${key}-label`}>{key==='energy'?'Energy':'Power'}</dt><dd key={`${key}-value`} data-energy-field={key}>{energyDisplay(result.energy!.fields[key])}</dd></>)}</dl>
          <details><summary>Counter details</summary><dl class="device-information">{(['energy','power'] as const).map(key=><><dt key={`${key}-raw-label`}>{key==='energy'?'Energy':'Power'} raw / scale</dt><dd key={`${key}-raw`}>{result.energy!.fields[key].raw??'Unavailable'} / {result.energy!.fields[key].scale??'Unavailable'}</dd></>)}</dl></details>
        </>:<p class="hint">{result.energy.format_status==='unrecognized'?`Unrecognized energy bank version ${result.energy.version}. Raw bytes are available below.`:'Energy data is unavailable from these readings.'}</p>}
      </section>}
      <details open={result.operation==='read_memory'}><summary>Captured bank {result.bank} bytes</summary><table class="memory-readings"><thead><tr><th>Location</th><th>Reading</th></tr></thead><tbody>{result.readings.map(item=><tr key={item.address}><td>0x{hex(item.address)}</td><td>{value(item.reading)}</td></tr>)}</tbody></table></details>
    </section>}
    <details key="memory-controls"><summary>Read a memory bank</summary><p class="hint">Read-only access. Unsupported banks or locations may return no response.</p><fieldset disabled={!!ui.busy} class="memory-window">
      <label class="field">Bank<input aria-label="Memory bank" type="number" inputMode="numeric" min="0" max="255" step="1" value={bank} onInput={e=>setBank(e.currentTarget.value)}/></label>
      <label class="field">First location<input aria-label="First memory location" type="number" inputMode="numeric" min="0" max="254" step="1" value={start} onInput={e=>setStart(e.currentTarget.value)}/></label>
      <label class="field">Byte count<input aria-label="Memory byte count" type="number" inputMode="numeric" min="1" max="64" step="1" value={count} onInput={e=>setCount(e.currentTarget.value)}/></label>
      <button disabled={!!ui.busy} onClick={()=>read('memory')}>Read memory</button>
    </fieldset></details>
    {row.kind==='output'&&<details key="energy-controls"><summary>Energy and power</summary><p class="hint">Read one standard energy bank as a captured snapshot. Changing the selection does not read the fixture.</p><fieldset disabled={!!ui.busy}>
      <label class="field">Energy bank<select aria-label="Energy bank" value={selectedEnergy} onChange={e=>setSelectedEnergy(e.currentTarget.value)}>{Object.entries(energyBanks).map(([bank,label])=><option key={bank} value={bank}>{label}</option>)}</select></label>
      <button disabled={!!ui.busy} onClick={()=>read('energy')}>Read energy data</button>
    </fieldset></details>}
    <div class="dialog-actions"><button onClick={dismiss}>Close</button>{row.kind==='output'&&<button disabled={!!ui.busy} onClick={()=>read('luminaire')}>Read luminaire data</button>}<button class="primary" disabled={!!ui.busy} onClick={()=>read()}>Read identity again</button></div>
  </Modal>;
}
