#!/usr/bin/env python3
"""Analyze saved CasinoGame Chrome traces; never launches a browser."""
import argparse
import collections
import gzip
import json
import pathlib
import statistics


def span_stats(events):
    durations = sorted(e['dur'] / 1000 for e in events)
    return {
        'count': len(events), 'wall_sum_ms': sum(durations),
        'thread_cpu_sum_ms': sum(e.get('tdur', 0) for e in events) / 1000,
        'median_ms': statistics.median(durations) if durations else None,
        'p95_ms': durations[int(len(durations) * .95)] if durations else None,
        'max_ms': max(durations) if durations else None,
    }


def analyze(root, result, metadata):
    path = root / result['traceFile']
    with (gzip.open(path, 'rt') if path.suffix == '.gz' else path.open()) as stream:
        events = json.load(stream)['traceEvents']
    start = next(e for e in events if e.get('name') == 'casino-sample-start')
    end = next(e for e in events if e.get('name') == 'casino-sample-end')
    low, high, pid = start['ts'], end['ts'], start['pid']
    assert high > low and end['pid'] == pid
    offset = low - start['args']['data']['startTime'] * 1000
    layer_ids = {e['args']['data']['layerTreeId'] for e in events
                 if e.get('name') == 'SetLayerTreeId' and e['pid'] == pid}
    assert len(layer_ids) == 1, 'Expected one game page layer tree'
    layer = next(iter(layer_ids))
    gpu = next(e['pid'] for e in events if e.get('name') == 'process_name'
               and e['args']['name'] == 'GPU Process')
    sample = [e for e in events if low <= e.get('ts', 0) <= high]

    # Async local IDs are reused: pair begin/end in export order within each process.
    pending, pairs = {}, []
    for event in events:
        if event.get('name') != 'PipelineReporter':
            continue
        key = (event['pid'], str(event.get('id2', event.get('id'))))
        if event['ph'] == 'b':
            assert key not in pending, 'Overlapping PipelineReporter local IDs'
            pending[key] = event
        elif event['ph'] == 'e' and key in pending:
            begin = pending.pop(key)
            assert event['ts'] >= begin['ts']
            pairs.append((begin, event))
    page_pairs = [(a, b) for a, b in pairs if a['pid'] == pid
                  and a['args']['frame_reporter']['layer_tree_host_id'] == layer
                  and low <= b['ts'] <= high]
    presented = [(a, b) for a, b in page_pairs
                 if a['args']['frame_reporter']['state'] in
                 ['STATE_PRESENTED_ALL', 'STATE_PRESENTED_PARTIAL']]
    # Main/forked reporters can refer to the same displayed compositor frame.
    display_ids = {a['args']['frame_reporter']['display_trace_id'] for a, b in presented}
    draw = [e for e in sample if e['name'] == 'DrawFrame' and e['pid'] == pid
            and e['args']['layerTreeId'] == layer]
    dropped = [e for e in sample if e['name'] == 'DroppedFrame' and e['pid'] == pid
               and e['args']['layerTreeId'] == layer]
    flushes = [e for e in events if e.get('ph') == 'X' and e['pid'] == gpu
               and e['tid'] == gpu and e['name'] == 'CommandBufferService:PutChanged'
               and e.get('args', {}).get('handler') == 'WebGPUDecoderImpl'
               and e['ts'] < high and e['ts'] + e.get('dur', 0) > low]
    long_flushes = [e for e in flushes if e['dur'] > 50000]
    main = [e for e in sample if e.get('ph') == 'X' and e['pid'] == pid
            and e['tid'] == pid and e['name'] == 'RunTask']
    raf = [e for e in sample if e.get('ph') == 'X' and e['pid'] == pid
           and e['tid'] == pid and e['name'] == 'FireAnimationFrame']
    times = result['sample']['times']
    worst = []
    for gap, a, b in sorted(((b-a, a, b) for a, b in zip(times, times[1:])), reverse=True)[:5]:
        gap_start, gap_end = offset + a * 1000, offset + b * 1000
        overlap = [e for e in flushes if e['ts'] < gap_end
                   and e['ts'] + e['dur'] > gap_start and e['dur'] > 10000]
        worst.append({'gap_ms': gap, 'sample_offset_ms': (gap_start-low) / 1000,
                      'overlapping_webgpu_flushes': [
                          {'sample_offset_ms': (e['ts']-low) / 1000,
                           'wall_ms': e['dur'] / 1000, 'thread_cpu_ms': e.get('tdur', 0) / 1000,
                           'overlap_ms': max(0, min(gap_end, e['ts']+e['dur']) -
                                             max(gap_start, e['ts'])) / 1000}
                          for e in overlap]})
    return {
        'directory': str(root), 'view': result['view'], 'path': str(path),
        'canvas': metadata['canvas'], 'setupKeys': metadata.get('setupKeys'),
        'conditions': result['sample']['conditions'], 'sample_trace_duration_ms': (high-low) / 1000,
        'renderer_pid': pid, 'layer_tree_id': layer,
        'acquisition_metrics': {k: v for k, v in result.items() if k not in ['sample', 'traceFile']},
        'page_compositor_draws': len(draw),
        'page_compositor_draw_unique_sequences': len({e['args']['frameSeqId'] for e in draw}),
        'page_dropped_event_count': len(dropped),
        'page_dropped_unique_sequences': len({e['args']['frameSeqId'] for e in dropped}),
        'pipeline_reporter_end_state_counts': dict(collections.Counter(
            a['args']['frame_reporter']['state'] for a, b in page_pairs)),
        'page_successful_unique_display_ids': len(display_ids),
        'page_successful_display_ids_per_second': len(display_ids) / ((high-low) / 1e6),
        'raw_browser_display_feedback_events': sum(e['name'] == 'Display::FrameDisplayed' for e in sample),
        'begin_frame_decision_reasons': dict(collections.Counter(
            e['args']['reason'] for e in sample if e['name'] == 'SendBeginFrameDecision')),
        'game_main_tasks': span_stats(main), 'game_raf_callbacks': span_stats(raf),
        'gpu_process_webgpu_flushes_over_50ms': span_stats(long_flushes),
        'gpu_process_long_flush_wall_clipped_to_sample_ms': sum(
            max(0, min(high, e['ts']+e['dur']) - max(low, e['ts'])) for e in long_flushes) / 1000,
        'worst_acquisition_gaps': worst,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directories', nargs='+', type=pathlib.Path)
    parser.add_argument('--output', required=True, type=pathlib.Path, help='Output prefix for .json and .txt')
    args = parser.parse_args()
    runs = []
    for root in args.directories:
        metadata = json.loads((root / 'results.json').read_text())
        runs.extend(analyze(root, result, metadata) for result in metadata['results'] if result.get('traceFile'))
    limits = [
        'Acquisitions are producer throughput, not physical presentation.',
        'Filter by marker renderer PID + page layer tree; deduplicate successful reporter display_trace_id.',
        'Successful page presentation feedback is compositor evidence, not unique fresh WebGPU texture-to-photon measurement.',
        'Display::FrameDisplayed also occurs for failed/sanitized feedback; its raw count is not game FPS.',
        'WebGPU decoder flush wall time includes driver/waits, not direct GPU shader execution time.',
        'Pair reporter spans by (PID, async local ID) in export order; filter by end timestamp inside sample marks.',
        'Other event counts use event timestamp inside marks. Long flush wall totals are clipped to marks; CPU totals describe full intersecting spans.',
        'Trace recording perturbs execution; compare ordinary throughput using separate untraced runs.',
    ]
    lines = []
    for run in runs:
        acquisition, flush, raf = (run[k] for k in ['acquisition_metrics', 'gpu_process_webgpu_flushes_over_50ms', 'game_raf_callbacks'])
        longest = f"{flush['max_ms']:.1f}ms" if flush['max_ms'] is not None else 'none'
        lines.extend([
            f"{run['directory']} / {run['view']}: {acquisition['fps']:.2f} acquisitions/s; {acquisition['max_gap_ms']:.1f}ms max gap; {run['page_compositor_draws']} page draws; {run['page_successful_unique_display_ids']} successful page display IDs ({run['page_successful_display_ids_per_second']:.2f}/s).",
            f"  WebGPU decoder flushes >50ms: {flush['count']}, {run['gpu_process_long_flush_wall_clipped_to_sample_ms']:.1f}ms clipped wall, {flush['thread_cpu_sum_ms']:.1f}ms full-span CPU, longest {longest}.",
            f"  Game RAF max {raf['max_ms']:.1f}ms; decision reasons {run['begin_frame_decision_reasons']}", '',
        ])
    args.output.with_suffix('.json').write_text(json.dumps({'metric_limits': limits, 'runs': runs}, indent=2) + '\n')
    args.output.with_suffix('.txt').write_text('\n'.join(lines + limits) + '\n')
    print('\n'.join(lines))


if __name__ == '__main__':
    main()
