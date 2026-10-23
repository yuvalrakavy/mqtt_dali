/** Driver-reported status, independent of brightness and color freshness. */
export const statusFlags = [
  ['GearFailure','Gear failure'],['LampFailure','Lamp failure'],['LampOn','Lamp on'],
  ['LimitError','Limit error'],['FadeRunning','Fade running'],['ResetState','Reset state'],
  ['MissingShortAddress','Missing short address'],['PowerFailure','Power failure'],
] as const;
export function fixtureStatus(state:Record<string,unknown>){
  const reported=state.FixtureStatusQuality;
  const quality=typeof reported==='string'&&['unobserved','current','unavailable','stale'].includes(reported)?reported:'unobserved';
  const raw=state.FixtureStatusRaw;
  const current=quality==='current'&&typeof raw==='number'&&Number.isInteger(raw)&&raw>=0&&raw<=255&&statusFlags.every(([key])=>typeof state[key]==='boolean');
  const faults=current?statusFlags.filter(([key])=>['GearFailure','LampFailure','LimitError','MissingShortAddress','PowerFailure'].includes(key)&&state[key]===true).map(([,label])=>label):[];
  const severity=!current?'unknown':state.GearFailure===true||state.LampFailure===true?'fault':faults.length?'warning':'clear';
  return {quality:quality==='current'&&!current?'unavailable':quality,raw:current?raw:null,severity,faults,
    flags:statusFlags.map(([key,label])=>({key,label,value:current?state[key] as boolean:null})),
    summary:!current?'Fixture status unknown':faults.length?faults.join(', '):'No reported faults'};
}
