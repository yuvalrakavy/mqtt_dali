import { Modal, useUi } from './ui.tsx';
import { Restoration, type RestorationReview } from './restoration.ts';

export function Recovery({scope,review,choose,close}:{scope:Restoration;review:RestorationReview|null;choose:(value:RestorationReview|null)=>void;close:()=>void}){
  const ui=useUi(),old=scope.original;
  const blink=ui.snapshot.identification,stopping=!!review?.candidate&&blink?.target===review.candidate.item.target&&['starting','blinking','restoring'].includes(blink.phase);
  const action=(work:()=>Promise<void>)=>{ui.cancelled=false;void ui.run(async()=>{ui.progress='Recovering saved fixture';ui.touch();try{await work();}finally{ui.progress='';ui.touch();}});};
  const select=()=>{try{choose(scope.review());}catch(error){ui.error=(error as Error).message;ui.touch();}};
  return <><div class="card recovery" role="region" aria-label="Lost address recovery">
    <h3>Recover {old.item.name} · previous address {old.item.short_address}</h3>
    <p>Keep this saved name, identity and bindings. Find and identify its new address on this list.</p>
    <div class="toolbar"><button disabled={!!ui.busy} onClick={()=>action(()=>scope.find())}>Find another candidate</button><button disabled={!!ui.busy} onClick={select}>Restore sole unaddressed fixture</button><button onClick={()=>{scope.cancelled=true;close();}}>Close recovery</button></div>
    <p class="hint">Candidates stay on this list if you close recovery. {scope.space==='outputs'?'Blink a candidate to identify it.':'Operate a candidate’s inputs and watch its current state and actions.'} Then choose Use this fixture on its module row.</p>
  </div>
  {review&&<Modal title="Confirm address restoration" close={()=>choose(null)}>
    {review.candidate?<>
      <p>Keep <strong>{old.item.name}</strong> and adopt address <strong>{review.candidate.item.short_address}</strong>, currently assigned to <strong>{review.candidate.item.name}</strong>.</p>
      <p>Previous address <strong>{old.item.short_address}</strong> becomes available again. The original saved fixture and its bindings continue.</p>
      <p>The selected row <strong>{review.candidate.item.name}</strong>{scope.space==='inputs'?' and its input instances':''} will be removed{scope.space==='outputs'?', including any of its saved group or scene references':''}.</p>
      {stopping&&<p class="hint">The candidate’s prior light level must be restored before adopting its address.</p>}
    </>:<>
      <p>Restore <strong>{old.item.name}</strong> at its previous address <strong>{old.item.short_address}</strong>.</p>
      <p>I associate the sole unaddressed fixture on this bus with this saved name.</p>
      <p>No address is programmed if none or multiple unaddressed fixtures are found. Use candidate identification, or deliberately isolate the intended fixture, if several lost their addresses.</p>
    </>}
    <div class="dialog-actions"><button onClick={()=>choose(null)}>Cancel</button><button class="primary" disabled={!!ui.busy} onClick={()=>{const selected=review;choose(null);action(async()=>{await selected.run();close();});}}>{review.candidate?(stopping?'Stop blink and adopt address':'Adopt identified address'):'Restore saved address'}</button></div>
  </Modal>}
  </>;
}
