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

def just_print(img):
    write_chunked(a='T', f=100, data=img)

def test_x_y_w_h_c_r(img):
    write_chunked(a='T', f=100, y=150, h=105, C=1, data=img)
    write_chunked(a='T', f=100, y=200, w=1, data=img)
    write_chunked(a='T', f=100, y=200, h=1, data=img)
    write_chunked(a='T', f=100, x=300, y=100, h=10, w=10, data=img)
    write_chunked(a='T', f=100, x=300, y=100, h=10, w=10, r=15, data=img)
    write_chunked(a='T', f=100, x=300, y=100, h=10, w=10, c=1, data=img)
    write_chunked(a='T', f=100, x=300, y=100, h=10, w=10, r=1, data=img)
    write_chunked(a='T', f=100, x=300, y=100, h=10, w=10, r=15, c=20, data=img)


def test_cell_offsets(img):
    write_chunked(a='T', f=100, h=10, w=10, X=2, Y=2, data=img)
    write_chunked(a='T', f=100, h=20, w=10, X=2, Y=2, data=img)
    write_chunked(a='T', f=100, h=2, Y=20, data=img)
    write_chunked(a='T', f=100, h=38, w=2, X=19, data=img)

if __name__ == "__main__":
    with open(sys.argv[-1], 'rb') as f:
        img = f.read()
    just_print(img)
    # test_x_y_w_h_c_r(img)
    # test_cell_offsets(img)
