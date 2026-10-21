"""Bounded native resource sampler. Only explicitly supplied test PIDs; never product polling."""
import ctypes,json,os,pathlib,signal,sys,time
class Usage(ctypes.Structure):
 _fields_=[('uuid',ctypes.c_ubyte*16)]+[(x,ctypes.c_uint64) for x in ['user','system','idle','interrupt','pageins','wired','resident','footprint','start','exit','child_user','child_system','child_idle','child_interrupt','child_pageins','child_elapsed','read','write']]
class Timebase(ctypes.Structure):_fields_=[('numer',ctypes.c_uint32),('denom',ctypes.c_uint32)]
lib=ctypes.CDLL('/usr/lib/libproc.dylib');tb=Timebase();assert ctypes.CDLL('/usr/lib/libSystem.B.dylib').mach_timebase_info(ctypes.byref(tb))==0
def children(pid):
 a=(ctypes.c_int*1024)();n=lib.proc_listchildpids(pid,a,ctypes.sizeof(a));assert n>=0
 return list(a)[:n]
def usage(pid):
 u=Usage();assert lib.proc_pid_rusage(pid,2,ctypes.byref(u))==0,pid
 return {'cpu_seconds':(u.user+u.system)*tb.numer/tb.denom/1e9,'footprint':u.footprint,'read':u.read,'write':u.write}
if __name__=='__main__':
 spec=json.loads(pathlib.Path(sys.argv[1]).read_text());pids=spec['pids'];producers=spec['producers'];samples=[]
 for p in producers:os.kill(p,signal.SIGUSR1)
 try:
  time.sleep(3);start=time.monotonic()
  for i in range(7):
   if i:time.sleep(max(0,start+i*5-time.monotonic()))
   samples.append({'elapsed':time.monotonic()-start,'processes':{k:usage(p) for k,p in pids.items()}})
 finally:
  for p in producers:os.kill(p,signal.SIGUSR1)
 result={**spec,'seconds':samples[-1]['elapsed'],'samples':samples,'results':{k:{'cpu_seconds':samples[-1]['processes'][k]['cpu_seconds']-samples[0]['processes'][k]['cpu_seconds'],'peak_MiB':max(s['processes'][k]['footprint'] for s in samples)/2**20} for k in pids}}
 result['total_cpu_seconds']=sum(r['cpu_seconds'] for r in result['results'].values());result['total_peak_MiB']=max(sum(p['footprint'] for p in s['processes'].values())/2**20 for s in samples)
 pathlib.Path(sys.argv[2]).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result[k] for k in ['seconds','results','total_cpu_seconds','total_peak_MiB']}))
