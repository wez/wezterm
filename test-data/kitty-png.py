#!python3
# This script encodes a PNG file using the kitty image protocol
import sys
from base64 import standard_b64encode

def serialize_gr_command(**cmd):
   payload = cmd.pop('payload', None)
   cmd = ','.join('{}={}'.format(k, v) for k, v in cmd.items())
   ans = []
   w = ans.append
   w(b'\033_G'), w(cmd.encode('ascii'))
   if payload:
      w(b';')
      w(payload)
   w(b'\033\\')
   return b''.join(ans)

def write_chunked(**cmd):
   data = standard_b64encode(cmd.pop('data'))
   while data:
      chunk, data = data[:4096], data[4096:]
      m = 1 if data else 0
      sys.stdout.buffer.write(serialize_gr_command(payload=chunk, m=m, **cmd))
      sys.stdout.flush()
      cmd.clear()

with open(sys.argv[-1], 'rb') as f:
   write_chunked(a='T', f=100, data=f.read())
