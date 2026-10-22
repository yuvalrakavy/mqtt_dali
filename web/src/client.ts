import { Views, rows, type Snapshot, type Job } from './model.ts';
export function decodeSnapshot(value:any):Snapshot {
  // The HTTP snapshot retains the published MQTT State property's envelope.
  const snapshot={...value,inventory:value.inventory.Inventory} as Snapshot;
  rows(snapshot);return snapshot;
}
export function continuation(initial:Snapshot,latest:Snapshot,revision:number,bus:number):boolean {
  const before=initial.inventory.buses.find(b=>b.bus===bus),after=latest.inventory.buses.find(b=>b.bus===bus);
  return initial.inventory.inventory_epoch===latest.inventory.inventory_epoch&&latest.inventory.revision===revision&&!!before&&!!after&&before.name===after.name&&after.enabled;
}
export class ApiError extends Error {status:number;constructor(status:number,message:string){super(message);this.status=status;}}
export class Client {
  views=new Views();csrf='';error='';events:any[]=[];lost=0;
  private source?:EventSource;private reconnect?:ReturnType<typeof setTimeout>;
  private stopped=false;private session=0;private activeAt=0;
  private requests=new Set<AbortController>();
  private sleepers=new Map<ReturnType<typeof setTimeout>,(error:Error)=>void>();
  private listeners=new Set<()=>void>();
  subscribe(listener:()=>void){this.listeners.add(listener);return()=>this.listeners.delete(listener);}
  notify(){for(const listener of this.listeners)listener();}
  private pause(ms:number):Promise<void>{return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{this.sleepers.delete(timer);resolve();},ms);this.sleepers.set(timer,reject);});}
  get snapshot():Snapshot|null{return this.views.value;}
  get jobs(){return this.views.jobBook;}
  async request(path:string,body?:unknown):Promise<any> {
    const session=this.session,controller=new AbortController();this.requests.add(controller);
    const timer=setTimeout(()=>controller.abort(),8000);
    try {
      const response=await fetch(path,{method:body===undefined?'GET':'POST',credentials:'same-origin',signal:controller.signal,
        headers:body===undefined?{}:{'Content-Type':'application/json','X-CSRF-Token':this.csrf},body:body===undefined?undefined:JSON.stringify(body)});
      if(session!==this.session||this.stopped)throw new Error('Session ended.');
      if(response.status===401){this.expired();throw new ApiError(401,'Sign in required.');}
      const value=response.status===204?null:await response.json();
      if(session!==this.session||this.stopped)throw new Error('Session ended.');
      if(!response.ok)throw new ApiError(response.status,value?.error??'The controller refused this request.');
      return value;
    } finally {clearTimeout(timer);this.requests.delete(controller);}
  }
  async start(){const session=this.session;const value=await this.request('/api/session');if(session!==this.session||this.stopped)return;this.csrf=value.csrf;this.connect();}
  private connect(){
    if(this.stopped)return;
    const epoch=this.views.begin(),session=this.session;
    this.source?.close();const source=new EventSource('/api/events');this.source=source;
    const current=()=>!this.stopped&&this.session===session&&this.views.epoch===epoch&&this.source===source;
    source.addEventListener('snapshot',event=>{if(!current())return;try {this.views.snapshot(epoch,decodeSnapshot(JSON.parse((event as MessageEvent).data)));this.error='';this.notify();}catch{this.error='Invalid controller snapshot.';this.views.ready=false;this.notify();}});
    source.addEventListener('jobs',event=>{if(!current())return;try{this.views.jobs(epoch,JSON.parse((event as MessageEvent).data).jobs);this.notify();}catch{this.error='Job updates are unavailable.';this.notify();}});
    source.addEventListener('input',event=>{if(!current())return;try{this.events.unshift({...JSON.parse((event as MessageEvent).data),received:Date.now()});this.events.length=Math.min(this.events.length,64);this.notify();}catch{this.lost++;this.notify();}});
    source.addEventListener('input_events_lost',event=>{if(!current())return;try{this.lost+=JSON.parse((event as MessageEvent).data).dropped;}catch{this.lost++;}this.notify();});
    source.onerror=()=>{
      if(!current())return;source.close();this.source=undefined;const ticket=this.views.begin();
      this.error='Connection interrupted. Reconnecting…';this.notify();
      const valid=()=>!this.stopped&&this.session===session&&this.views.epoch===ticket;
      const retry=async()=>{if(!valid())return;try {const access=await this.request('/api/session');if(valid()){this.csrf=access.csrf;this.connect();}}
        catch{if(valid()){this.error='Controller is unavailable.';this.notify();this.reconnect=setTimeout(retry,2000);}}};
      this.reconnect=setTimeout(retry,1000);
    };
  }
  activity(){const now=Date.now();if(now-this.activeAt<60000||!this.csrf||this.stopped)return;this.activeAt=now;void this.request('/api/session/activity',{}).catch(()=>{});}
  async job(generation:string,target:string|null,command:string,params:unknown={}):Promise<any> {
    if(!this.views.ready)throw new Error('Wait for a current controller view.');
    const session=this.session;
    const admission=await this.request('/api/jobs',{generation,target,command,params});
    const id=admission.id,deadline=Date.now()+25000;
    if(typeof id!=='string')throw new Error('Controller did not return a job id.');
    while(Date.now()<deadline) {
      if(session!==this.session||this.stopped)throw new Error('Session ended; inspect the job before retrying.');
      let job=this.views.jobBook.find(j=>j.id===id);
      // HTTP result lookup belongs only to this waiter, never to streamed views.
      if(!job||job.status==='running'||(job.status==='succeeded'&&job.result==null))job=await this.request(`/api/jobs/${id}`);
      if(job?.status==='succeeded')return job.result;
      if(job?.status==='failed')throw new Error(job.error?.message??'Operation failed. Inspect current state before retrying.');
      await this.pause(50);
    }
    throw new Error('Result is not available yet. Inspect Jobs before retrying.');
  }
  async atRevision(revision:number):Promise<Snapshot> {
    const until=Date.now()+5000,session=this.session;
    while(Date.now()<until){if(this.stopped||session!==this.session)throw new Error('Session ended.');const value=this.snapshot;if(this.views.ready&&value&&value.inventory.revision>=revision)return value;await this.pause(20);}
    throw new Error('Waiting for the updated inventory. Refresh before another change.');
  }
  async logout(){await this.request('/api/logout',{});this.expired();}
  private expired(){this.close();location.assign('/login');}
  close(){this.stopped=true;this.session++;this.source?.close();if(this.reconnect)clearTimeout(this.reconnect);for(const request of this.requests)request.abort();this.requests.clear();for(const [timer,reject] of this.sleepers){clearTimeout(timer);reject(new Error('Session ended.'));}this.sleepers.clear();this.views.clear();this.csrf='';this.events=[];this.notify();}
}
