import {useEffect,useLayoutEffect,useState} from 'preact/hooks';
import {Modal,useUi} from './ui.tsx';
import type {PhysicalRow} from './model.ts';
import {DeviceInfoScope,memoryWindow,type MemoryReport,type MemoryReading} from './device-info.ts';
const hex=(v:number)=>v.toString(16).toUpperCase().padStart(2,'0');
const value=(r:MemoryReading|null)=>r===null?'Not read':r.kind==='value'?`0x${hex(r.raw)} (${r.raw})`:r.kind==='no_response'?'No response':r.kind==='receive_collision'?'Receive collision':'Transmit collision';
export function DeviceInfo({row,close}:{row:PhysicalRow;close:()=>void}){
  const ui=useUi(),[scope]=useState(()=>new DeviceInfoScope(ui,row));
  const [result,setResult]=useState<MemoryReport|null>(null),[error,setError]=useState(''),[bank,setBank]=useState('0'),[start,setStart]=useState('0'),[count,setCount]=useState('16');
  useLayoutEffect(()=>()=>{scope.cancelled=true;},[scope]);
  const dismiss=()=>{scope.cancelled=true;close();};
  const read=(raw=false)=>void ui.run(async()=>{
    setResult(null);setError('');
    try{
      const number=(text:string)=>text.trim()===''?NaN:Number(text);
      const window=raw?memoryWindow(number(bank),number(start),number(count)):undefined;
      const r=await scope.read(window);if(!scope.cancelled)setResult(r);
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
      <details open={result.operation==='read_memory'}><summary>Captured bank {result.bank} bytes</summary><table class="memory-readings"><thead><tr><th>Location</th><th>Reading</th></tr></thead><tbody>{result.readings.map(item=><tr key={item.address}><td>0x{hex(item.address)}</td><td>{value(item.reading)}</td></tr>)}</tbody></table></details>
    </section>}
    <details><summary>Read a memory bank</summary><p class="hint">Read-only access. Unsupported banks or locations may return no response.</p><fieldset disabled={!!ui.busy} class="memory-window">
      <label class="field">Bank<input aria-label="Memory bank" type="number" inputMode="numeric" min="0" max="255" step="1" value={bank} onInput={e=>setBank(e.currentTarget.value)}/></label>
      <label class="field">First location<input aria-label="First memory location" type="number" inputMode="numeric" min="0" max="254" step="1" value={start} onInput={e=>setStart(e.currentTarget.value)}/></label>
      <label class="field">Byte count<input aria-label="Memory byte count" type="number" inputMode="numeric" min="1" max="64" step="1" value={count} onInput={e=>setCount(e.currentTarget.value)}/></label>
      <button disabled={!!ui.busy} onClick={()=>read(true)}>Read memory</button>
    </fieldset></details>
    <div class="dialog-actions"><button onClick={dismiss}>Close</button><button class="primary" disabled={!!ui.busy} onClick={()=>read()}>Read identity again</button></div>
  </Modal>;
}
