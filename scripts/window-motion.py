"""Native mouse/geometry/composited-pixel replay on a disposable hosted Windows runner only."""

import argparse
import bisect
import csv
import ctypes
from ctypes import wintypes as win
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import threading
import time


def hosted_only():
    if not (sys.platform == 'win32' and os.environ.get('GITHUB_ACTIONS') == 'true'
            and os.environ.get('MOTION_RUNNER') == 'github-hosted' and os.environ.get('RUNNER_OS') == 'Windows'
            and os.environ.get('GITHUB_RUN_ID', '').isdigit() and os.environ.get('RUNNER_TEMP')
            and os.environ.get('GITHUB_REPOSITORY', '').lower() == 'lianzy-baimiao/fluentcal'):
        raise SystemExit('Refused: native input is only available on the project\'s GitHub-hosted Windows runner.')


def percentile(values, fraction=.95):
    return round(sorted(values)[max(0, math.ceil(len(values) * fraction) - 1)], 2) if values else None


def contrast(first, second, position):
    pos = position * 4
    return sum(abs(first[pos + channel] - second[pos + channel]) for channel in range(3))


def visible_edge(background, filled, line):
    columns = [pos for pos in range(len(line) // 4) if contrast(background, filled, pos) >= 30]
    if len(columns) < .65 * (len(line) // 4):
        return None
    changed = [contrast(background, line, pos) >= min(60, max(18, contrast(background, filled, pos) * .45)) for pos in columns]
    errors = [sum(changed)]
    for occupied in changed:
        errors.append(errors[-1] + (-1 if occupied else 1))
    best = min(errors)
    if best > len(columns) * .15:
        return None
    cuts = [columns[index] if index < len(columns) else columns[-1] + 1 for index, score in enumerate(errors) if score == best]
    return statistics.median(cuts)


def crossing_delays(requests, samples, span, tolerance):
    times = [stamp for stamp,_ in samples]
    delays, missed = [], 0
    for number, (sent, old, target) in enumerate(requests):
        if not span*.15 < target < span*.85: continue
        direction = 1 if target>old else -1
        deadline = min(sent+200, next((when for when,start,end in requests[number+1:] if (end-start)*direction<0), sent+200))
        delay = next((when-sent for when,value in samples[bisect.bisect_left(times,sent):bisect.bisect_right(times,deadline)] if value is not None and direction*(value-target)>=-tolerance), None)
        if delay is None: missed+=1
        else: delays.append(delay)
    return percentile(delays), missed


def self_test():
    dark, light = bytes([20, 30, 40, 0]), bytes([190, 200, 180, 0])
    background, filled = dark * 120, light * 120
    for position in (0, 10, 45, 60, 105, 120):
        assert visible_edge(background, filled, light * position + dark * (120-position)) == position
    assert visible_edge(background, background, filled) is None
    assert visible_edge(background, filled, (light + dark) * 60) is None
    assert percentile([0, 0, 5, 20]) == 20 and percentile([]) is None
    assert crossing_delays([(10,10,30),(30,30,50)],[(15,10),(22,30),(43,50)],100,1)==(13,0)
    assert crossing_delays([(10,10,30)],[(35,None)],100,1)==(None,1)
    print('Offline edge, ambiguous-frame and quantile tests passed; no desktop APIs called.')


def api(library, name, result, *params):
    function = getattr(library, name)
    function.restype, function.argtypes = result, params
    return function


class GuiInfo(ctypes.Structure):
    _fields_ = [('size', win.DWORD), ('flags', win.DWORD)] + [(name, win.HWND) for name in ('active','focus','capture','menu','moving','caret')] + [('rect', win.RECT)]


class Mouse(ctypes.Structure):
    _fields_ = [('dx', win.LONG), ('dy', win.LONG), ('data', win.DWORD), ('flags', win.DWORD), ('time', win.DWORD), ('extra', ctypes.c_size_t)]


class Input(ctypes.Structure):
    _fields_ = [('kind', win.DWORD), ('mouse', Mouse)]


class Desktop:
    def __init__(self):
        hosted_only()
        import win32api, win32gui, win32process
        self.gui, self.system, self.processes = win32gui, win32api, win32process
        self.user = ctypes.WinDLL('user32', use_last_error=True)
        self.user.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
        session = win.DWORD()
        if not ctypes.windll.kernel32.ProcessIdToSessionId(os.getpid(), ctypes.byref(session)) or not session.value:
            raise RuntimeError('No interactive runner session; refusing to use Session 0.')
        if self.user.GetSystemMetrics(80) != 1 or self.user.GetSystemMetrics(0) < 800:
            raise RuntimeError('Expected one usable guest screen')
        self.width, self.height = (self.user.GetSystemMetrics(index) for index in (0, 1))
        self.send = api(self.user, 'SendInput', win.UINT, win.UINT, ctypes.POINTER(Input), ctypes.c_int)
        self.gui_info = api(self.user, 'GetGUIThreadInfo', win.BOOL, win.DWORD, ctypes.POINTER(GuiInfo))
        self.full_drag = win.BOOL()
        if not self.user.SystemParametersInfoW(38, 0, ctypes.byref(self.full_drag), 0):
            raise ctypes.WinError()
        print(f'Hosted session={session.value}, screen={self.width}x{self.height}, full-window drag={self.full_drag.value}', flush=True)

    def mouse(self, flags, point=None):
        dx = dy = 0
        if point is not None:
            dx, dy = (round((value + .5) * 65536 / size) for value, size in zip(point, (self.width, self.height)))
            flags |= 0x8000 | 0x4000
        event = Input(0, Mouse(dx, dy, 0, flags, 0, 0))
        if self.send(1, ctypes.byref(event), ctypes.sizeof(event)) != 1:
            raise ctypes.WinError(ctypes.get_last_error())

    def moving(self, thread, hwnd):
        info = GuiInfo(size=ctypes.sizeof(GuiInfo))
        return bool(self.gui_info(thread, ctypes.byref(info)) and info.flags & 2 and info.moving == hwnd)

    def window(self, process, title):
        for _ in range(200):
            found = []
            def visit(hwnd, _):
                if self.gui.IsWindowVisible(hwnd) and self.gui.GetWindowText(hwnd) == title and self.processes.GetWindowThreadProcessId(hwnd)[1] == process.pid:
                    found.append(hwnd)
                return True
            self.gui.EnumWindows(visit, None)
            if found:
                return found[0]
            if process.poll() is not None:
                raise RuntimeError(f'{title} exited early: {process.returncode}')
            time.sleep(.1)
        raise RuntimeError(f'{title} window not visible')


class Strip:
    def __init__(self, desktop, origin, length, vertical):
        self.origin, self.width, self.height = origin, (1 if vertical else length), (length if vertical else 1)
        self.gdi = ctypes.WinDLL('gdi32', use_last_error=True)
        handle, integer, pointer = win.HANDLE, ctypes.c_int, ctypes.c_void_p
        self.release_dc = api(desktop.user, 'ReleaseDC', integer, handle, handle)
        self.screen = api(desktop.user, 'GetDC', handle, handle)(None)
        self.dest = api(self.gdi, 'CreateCompatibleDC', handle, handle)(self.screen)
        import struct
        header = struct.pack('<IiiHHIIiiII', 40, self.width, -self.height, 1, 32, 0, 0, 0, 0, 0, 0)
        self.bits = pointer()
        self.bitmap = api(self.gdi, 'CreateDIBSection', handle, handle, pointer, win.UINT, ctypes.POINTER(pointer), handle, win.DWORD)(self.dest, header, 0, ctypes.byref(self.bits), None, 0)
        self.select = api(self.gdi, 'SelectObject', handle, handle, handle)
        self.old = self.select(self.dest, self.bitmap)
        self.copy = api(self.gdi, 'BitBlt', win.BOOL, handle, integer, integer, integer, integer, handle, integer, integer, win.DWORD)
        self.delete = api(self.gdi, 'DeleteObject', win.BOOL, handle)
        self.delete_dc = api(self.gdi, 'DeleteDC', win.BOOL, handle)
        if not all((self.screen, self.dest, self.bitmap, self.bits.value, self.old)):
            raise RuntimeError('Cannot allocate desktop pixel strip')

    def read(self):
        if not self.copy(self.dest, 0, 0, self.width, self.height, self.screen, *self.origin, 0x40CC0020) or not self.gdi.GdiFlush():
            raise ctypes.WinError(ctypes.get_last_error())
        return ctypes.string_at(self.bits, self.width * self.height * 4)

    def close(self):
        self.select(self.dest, self.old); self.delete(self.bitmap); self.delete_dc(self.dest); self.release_dc(None, self.screen)


class PresentedStrip:
    def __init__(self, origin, span, vertical):
        hosted_only()
        library = ctypes.CDLL(str(Path(__file__).resolve().with_name('window-motion-frame.dll')))
        handle = ctypes.c_void_p
        self.handle = handle()
        opener = api(library, 'open_strip', ctypes.c_long, ctypes.c_int, ctypes.c_int, win.UINT, win.UINT, ctypes.POINTER(handle))
        status = opener(*origin, 1 if vertical else span, span if vertical else 1, ctypes.byref(self.handle))
        if status:
            raise OSError(f'Desktop Duplication unavailable: 0x{status & 0xffffffff:08x}')
        self.next = api(library,'next_strip',ctypes.c_long,handle,handle,ctypes.POINTER(ctypes.c_longlong),ctypes.POINTER(win.UINT))
        self.release = api(library,'close_strip',None,handle)
        self.buffer = ctypes.create_string_buffer(span*4)
        frequency, tick = ctypes.c_longlong(), ctypes.c_longlong()
        ctypes.windll.kernel32.QueryPerformanceFrequency(ctypes.byref(frequency))
        before=time.perf_counter(); ctypes.windll.kernel32.QueryPerformanceCounter(ctypes.byref(tick)); after=time.perf_counter()
        self.frequency, self.offset = frequency.value, (before+after)/2 - tick.value/frequency.value

    def read(self):
        stamp, count = ctypes.c_longlong(), win.UINT()
        result = self.next(self.handle,self.buffer,ctypes.byref(stamp),ctypes.byref(count))
        if result == 1: return None
        if result != 0: raise OSError(f'Desktop Duplication read failed: 0x{result & 0xffffffff:08x}')
        return self.offset+stamp.value/self.frequency, count.value, self.buffer.raw

    def close(self): self.release(self.handle)


def wait_until(deadline):
    remaining = deadline - time.perf_counter()
    if remaining > 0:
        time.sleep(remaining)


def measure(desktop, hwnd, reference, operation, directory):
    gui = desktop.gui
    initial = gui.GetWindowRect(hwnd)
    left, top, right, bottom = initial
    vertical = operation == 'resize'
    axis, edge = (1, 3) if vertical else (0, 2)
    span = min(120, (desktop.height - bottom - 55) if vertical else (desktop.width - right - 30)) // 12 * 12
    if span < 48:
        raise RuntimeError(f'Not enough room for a move/resize gesture: {initial}, span={span}')
    mid_x = (left + right) // 2
    dpi = api(desktop.user, 'GetDpiForWindow', win.UINT, win.HWND)(hwnd)
    point = (mid_x, bottom - round(14*dpi/96) if vertical else (top + bottom) // 2)
    if reference:
        code = 15 if vertical else 2
        candidates = range(bottom - 1, bottom - 20, -1) if vertical else range(top + 1, top + 50)
        point = next(((mid_x, ypos) for ypos in candidates if gui.SendMessage(hwnd, 0x84, 0, (ypos << 16) | mid_x) == code), None)
        if not point:
            raise RuntimeError(f'No native hit code {code} on control')
    hit = gui.WindowFromPoint(point)
    if gui.GetAncestor(hit, 2) != hwnd:
        raise RuntimeError(f'Test point occluded by {gui.GetClassName(hit)} at {point}; not sending mouse input')
    thread, _ = desktop.processes.GetWindowThreadProcessId(hwnd)
    origin = (int(left + (right-left)*.32), bottom) if vertical else (right, top + (bottom-top)//3)
    strip = Strip(desktop, origin, span, vertical)
    schedule = [('near', 0)] * 10
    for _ in range(3):
        for steps, dwell in [(range(1, 7), 'mid'), (range(7, 13), 'far'), (range(11, -1, -1), 'near')]:
            offsets = [step * span // 12 for step in steps]
            schedule += [('moving', offset) for offset in offsets] + [(dwell, offsets[-1])] * 12
    rows, frames, requests, failures = [], [], [], []
    state = [('near', 0, 0.0)]
    desktop.mouse(1, point)
    time.sleep(.12)
    try: baseline = strip.read()
    finally: strip.close()
    try:
        capture = PresentedStrip(origin, span, vertical)
        first = capture.read()
        if first: baseline = first[2]
    except OSError as error:
        print('No presentation timestamps:', error, flush=True)
        capture = None
    stop_capture = threading.Event()
    started = time.perf_counter()
    frames.append((0., 1, baseline))
    def collect():
        try:
            while not stop_capture.is_set():
                frame = capture.read()
                if frame:
                    stamp, count, image = frame
                    if stamp >= started: frames.append(((stamp-started)*1000, count, image))
        except OSError as error: print('Presentation collection stopped:', error, flush=True)
    reader = threading.Thread(target=collect) if capture else None
    def drive():
        try:
            previous = 0
            for index, (phase, offset) in enumerate(schedule):
                wait_until(started + index / 60)
                if previous != offset:
                    target = list(point); target[axis] += offset
                    before = time.perf_counter()
                    desktop.mouse(1, target)
                    requests.append(((before-started)*1000, previous, offset))
                if previous != offset or state[-1][0] != phase:
                    state.append((phase, offset, (time.perf_counter()-started)*1000))
                previous = offset
        except Exception as error:
            failures.append(str(error))
    driver = threading.Thread(target=drive)
    desktop.mouse(2)
    try:
        if reader: reader.start()
        driver.start()
        while driver.is_alive():
            begin = (time.perf_counter()-started)*1000
            phase, offset, changed = state[-1]
            pos = desktop.system.GetCursorPos()[axis] - point[axis]
            rect = gui.GetWindowRect(hwnd)
            native = desktop.moving(thread, hwnd)
            rows.append([begin, (time.perf_counter()-started)*1000, phase, offset, begin-changed, pos, rect[edge]-initial[edge], int(native), *rect])
            time.sleep(.002)
    finally:
        driver.join(6); desktop.mouse(4); stop_capture.set()
        if reader:
            reader.join(); capture.close()
    time.sleep(.15)
    if failures:
        raise RuntimeError(failures)
    directory.mkdir(parents=True, exist_ok=True)
    (directory/'scan.bgra').write_bytes(b''.join(frame[2] for frame in frames))
    (directory/'presentation.json').write_text(json.dumps([(stamp,count) for stamp,count,_ in frames]))
    (directory/'background.bgra').write_bytes(baseline)
    (directory/'requests.json').write_text(json.dumps(requests))
    stable = [index for index, row in enumerate(rows) if row[2]=='far' and row[4]>90 and abs(row[6]-span)<=2]
    times = [frame[0] for frame in frames]
    indexes = [max(0,bisect.bisect_right(times,row[0])-1) for row in rows]
    far = frames[indexes[stable[len(stable)//2]]][2] if stable else baseline
    detected = [visible_edge(baseline,far,frame[2]) for frame in frames]
    estimates = [detected[index] if index+1==len(frames) or frames[index+1][1]<=1 else None for index in indexes]
    middle = [estimate-row[6] for row, estimate in zip(rows, estimates) if estimate is not None and row[2]=='mid' and row[4]>90 and abs(row[6]-span/2)<=2]
    bias = statistics.median(middle) if middle else None
    measured, visual = [], []
    with (directory/'samples.csv').open('w',newline='') as output:
        writer = csv.writer(output); writer.writerow(('begin_ms','end_ms','phase','requested_px','phase_age_ms','cursor_px','hwnd_px','native','left','top','right','bottom','visible_px'))
        for row, estimate in zip(rows, estimates):
            visible = estimate-bias if bias is not None and estimate is not None else None
            writer.writerow([*row, visible])
            if row[2]=='moving' and row[7] and span*.15 < row[5] < span*.85:
                measured.append(abs(row[5]-row[6]))
                if visible is not None:
                    visual.append(abs(row[5]-visible))
    anchors = (all(abs(row[8]-left)<=2 and abs(row[9]-top)<=2 for row in rows) if vertical else
               all(abs(row[10]-row[8]-(right-left))<=2 and abs(row[9]-top)<=2 and abs(row[11]-bottom)<=2 for row in rows))
    hwnd_delay, hwnd_misses = crossing_delays(requests, [(row[0],row[6]) for row in rows],span,1)
    pixel_ok = len(visual)>=.8*len(measured) and len(middle)>=5 and bool(measured)
    pixel_delay, pixel_misses = crossing_delays(requests, [(stamp, detected[index]-bias if detected[index] is not None else None) for index,stamp in enumerate(times)],span,2) if pixel_ok else (None,None)
    result = dict(operation=operation, span_px=span, samples=len(rows), input_steps=len(requests), modal_samples=sum(row[7] for row in rows), moving_samples=len(measured), pixel_samples=len(visual),
                  input_to_hwnd_p95_ms=hwnd_delay, hwnd_missed_thresholds=hwnd_misses, input_to_composed_p95_ms=pixel_delay, pixel_missed_thresholds=pixel_misses,
                  presented_frames=len(frames)-1, skipped_presents=sum(max(0,frame[1]-1) for frame in frames),
                  capture_p95_ms=percentile([row[1]-row[0] for row in rows]), sample_interval_p95_ms=percentile([second[0]-first[0] for first,second in zip(rows,rows[1:])]),
                  hwnd_offset_p95_px=percentile(measured), pixel_offset_p95_px=percentile(visual) if pixel_ok else None, pixel_bias_px=bias)
    print(directory.name, json.dumps(result), flush=True)
    if not anchors or len(measured)<25 or not stable or max(abs(actual-want) for actual,want in zip(gui.GetWindowRect(hwnd),initial))>3:
        raise RuntimeError(f'Invalid or incomplete native gesture: {result}; anchors={anchors}; final={gui.GetWindowRect(hwnd)} initial={initial}')
    return result


def fixture(view, width, height):
    tasks = {}
    if view == 'list':
        for number in range(40):
            day = number % 28 + 1
            history = {}
            for offset in range(36):
                year, month = divmod(2026*12+9-offset,12)
                slot = f'{year:04}-{month+1:02}'
                history[slot] = f'{slot}-{day:02}'
            key = f'task-{number}'
            tasks[key] = dict(id=key,title=key,start=f'2023-11-{day:02}',end=f'2023-11-{day:02}',flex='month',flexDone=history,order=number,createdAt=1,updatedAt=1)
    return dict(version=1, tasks=tasks, groups={}, holidays={}, dayMarks={}, settings=dict(general=dict(seeded=True,onboarded=True,language='en-US'),
                desk=dict(locked=False,pinMode='desktop',noActivate=True,bounds=dict(x=50,y=50,w=width,h=height)), calendar=dict(view=view,sidebar=False,fitRows=view=='list',listRange='all'),
                tasks=dict(showCompleted=True),shortcuts=dict(toggle='', newTask=''),holidays=dict(autoUpdate=False),reminders=dict(enabled=False)))


def control(width, height):
    hosted_only()
    ctypes.windll.user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
    import tkinter
    root = tkinter.Tk(); root.title('FluentCal motion control'); root.geometry(f'{width}x{height}+50+50'); root.configure(background='#588ab4'); root.mainloop()


def experiment(args):
    desk = Desktop()
    width, height = min(1800,desk.width-260), min(1100,desk.height-245)
    args.output.mkdir(parents=True,exist_ok=True)
    builds = [('stock',args.stock)] + ([('candidate',args.candidate)] if args.candidate else [])
    summary = []
    errors = []
    try:
        if not desk.full_drag.value and not desk.user.SystemParametersInfoW(37,1,None,2):
            raise RuntimeError('Could not enable full-window drawing in the disposable runner')
        for repeat in range(3):
            groups = [('control','control',None)] + [(label,view,binary) for label,binary in builds for view in ('month','list')]
            for label,view,binary in (reversed(groups) if repeat%2 else groups):
                name=f'{repeat+1}-{label}-{view}'; env=os.environ.copy()
                with tempfile.TemporaryDirectory(prefix='fluentcal-motion-') as folder:
                    if binary:
                        Path(folder,'data.json').write_text(json.dumps(fixture(view,width,height)),encoding='utf-8')
                        env.update(FLUENTCAL_DATA_DIR=folder, FLUENTCAL_INSTANCE='FluentCal.MotionCI.'+name)
                    command = [str(binary.resolve())] if binary else [sys.executable,__file__,'--control',str(width),str(height)]
                    proc = subprocess.Popen(command,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
                    try:
                        if args.one_core:
                            allowed,_ = desk.processes.GetProcessAffinityMask(int(proc._handle)); desk.processes.SetProcessAffinityMask(int(proc._handle),allowed & -allowed)
                        hwnd = desk.window(proc,'FluentCal' if binary else 'FluentCal motion control')
                        if binary:
                            ipc=desk.gui.FindWindow(env['FLUENTCAL_INSTANCE']+'-sic',None)
                            if ipc: desk.gui.PostMessage(ipc,0x8000+0x51,0,0x202)
                        time.sleep(.65)
                        for operation in ('move','resize'):
                            trial=name+'-'+operation
                            try:
                                metrics=measure(desk,hwnd,not binary,operation,args.output/trial)
                                summary.append(dict(trial=trial,**metrics))
                            except Exception as error:
                                errors.append([trial,str(error)]); print('INVALID',trial,str(error),flush=True)
                    finally:
                        if proc.poll() is None: proc.terminate()
                        proc.wait(10)
                    (args.output/'summary.json').write_text(json.dumps(dict(results=summary,errors=errors),indent=2))
    finally:
        if not desk.full_drag.value: desk.user.SystemParametersInfoW(37,0,None,2)
    if errors: raise RuntimeError(f'{len(errors)} invalid gestures; see summary.json')


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--self-test',action='store_true'); parser.add_argument('--control',nargs=2,type=int)
    parser.add_argument('--stock',type=Path); parser.add_argument('--candidate',type=Path); parser.add_argument('--one-core',action='store_true'); parser.add_argument('--output',type=Path)
    options=parser.parse_args()
    if options.self_test: self_test()
    elif options.control: control(*options.control)
    elif options.stock and options.output: experiment(options)
    else: parser.error('specify --self-test or --stock and --output')
