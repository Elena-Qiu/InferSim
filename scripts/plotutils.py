#
# Copyright 2019 Peifeng Yu <peifeng@umich.edu>
#
# This file is part of Salus
# (see https://github.com/SymbioticLab/Salus).
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#    http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
from __future__ import division, print_function, absolute_import, annotations

from contextlib import contextmanager
from os.path import getsize, basename

import pandas as pd
import numpy as np
from cycler import cycler
import itertools

import matplotlib as mpl
import matplotlib.pyplot as plt
import matplotlib.transforms as mtransforms
import matplotlib.ticker as mticker
from matplotlib.dates import SECONDLY, rrulewrapper, RRuleLocator, DateFormatter
from matplotlib.collections import LineCollection
from matplotlib.path import Path as mPath

from typing import TYPE_CHECKING
if TYPE_CHECKING:
    from typing import Sequence, List


def default_marker_begin():
    return mPath([
        (-0.5, 0.866),
        (0, 0),
        (0, 1.0),
        (0, -1.0),
        (0, 0),
        (-0.5, -0.866),
        (0, 0),
    ])


def default_marker_end():
    return mPath([
        (0.5, 0.866),
        (0, 0),
        (0, 1.0),
        (0, -1.0),
        (0, 0),
        (0.5, -0.866),
        (0, 0),
    ])


# http://stackoverflow.com/q/3844931/
def check_equal(lst):
    '''
    Return True if all elements in the list are equal
    '''
    return not lst or [lst[0]]*len(lst) == lst


def gen_groupby(*args: pd.Series, groups: List[pd.Series]):
    '''
    group args by groups.
    Each args should be list-like, groups should be a list of list-like.
    Each of them should be of the same length
    '''
    if len(args) == 0 or len(groups) == 0:
        raise ValueError('args or groups must be non-empty')

    groups = tuple(groups)
    lens = [len(col) for col in args + groups]
    if not check_equal(lens):
        raise ValueError(f'args + groups of different length, got {lens}')

    # create a dataframe from group keys
    groups: pd.DataFrame = pd.concat(groups, axis=1)
    keys = groups.drop_duplicates()

    for _, grp_key in keys.iterrows():
        mask = (groups == grp_key).all(axis=1)
        yield grp_key, [arg[mask] for arg in args]


SYMBOLS = {
    'customary': ('B', 'K', 'M', 'G', 'T', 'P', 'E', 'Z', 'Y'),
    'customary_ext': ('byte', 'kilo', 'mega', 'giga', 'tera', 'peta', 'exa',
                      'zetta', 'iotta'),
    'iec': ('Bi', 'Ki', 'Mi', 'Gi', 'Ti', 'Pi', 'Ei', 'Zi', 'Yi'),
    'iec_ext': ('byte', 'kibi', 'mebi', 'gibi', 'tebi', 'pebi', 'exbi',
                'zebi', 'yobi'),
}


def bytes2human(n, format='%(value).1f %(symbol)s', symbols='customary'):
    """
    Convert n bytes into a human readable string based on format.
    symbols can be either "customary", "customary_ext", "iec" or "iec_ext",
    see: http://goo.gl/kTQMs

      >>> bytes2human(0)
      '0.0 B'
      >>> bytes2human(0.9)
      '0.0 B'
      >>> bytes2human(1)
      '1.0 B'
      >>> bytes2human(1.9)
      '1.0 B'
      >>> bytes2human(1024)
      '1.0 K'
      >>> bytes2human(1048576)
      '1.0 M'
      >>> bytes2human(1099511627776127398123789121)
      '909.5 Y'

      >>> bytes2human(9856, symbols="customary")
      '9.6 K'
      >>> bytes2human(9856, symbols="customary_ext")
      '9.6 kilo'
      >>> bytes2human(9856, symbols="iec")
      '9.6 Ki'
      >>> bytes2human(9856, symbols="iec_ext")
      '9.6 kibi'

      >>> bytes2human(10000, "%(value).1f %(symbol)s/sec")
      '9.8 K/sec'

      >>> # precision can be adjusted by playing with %f operator
      >>> bytes2human(10000, format="%(value).5f %(symbol)s")
      '9.76562 K'
    """
    n = int(n)
    sign = ''
    if n < 0:
        sign = '-'
        n = -n
    symbols = SYMBOLS[symbols]
    prefix = {}
    for i, s in enumerate(symbols[1:]):
        prefix[s] = 1 << (i + 1) * 10
    for symbol in reversed(symbols[1:]):
        if n >= prefix[symbol]:
            value = float(n) / prefix[symbol]
            return sign + format % locals()
    return sign + format % dict(symbol=symbols[0], value=n)


def human2bytes(s):
    """
    Attempts to guess the string format based on default symbols
    set and return the corresponding bytes as an integer.
    When unable to recognize the format ValueError is raised.

      >>> human2bytes('0 B')
      0
      >>> human2bytes('1 K')
      1024
      >>> human2bytes('1 M')
      1048576
      >>> human2bytes('1 Gi')
      1073741824
      >>> human2bytes('1 tera')
      1099511627776

      >>> human2bytes('0.5kilo')
      512
      >>> human2bytes('0.1  byte')
      0
      >>> human2bytes('1 k')  # k is an alias for K
      1024
      >>> human2bytes('12 foo')
      Traceback (most recent call last):
          ...
      ValueError: can't interpret '12 foo'
    """
    init = s
    num = ""
    while s and s[0:1].isdigit() or s[0:1] == '.':
        num += s[0]
        s = s[1:]
    num = float(num)
    letter = s.strip()
    for name, sset in SYMBOLS.items():
        if letter in sset:
            break
    else:
        if letter == 'k':
            # treat 'k' as an alias for 'K' as per: http://goo.gl/kTQMs
            sset = SYMBOLS['customary']
            letter = letter.upper()
        else:
            raise ValueError("can't interpret %r" % init)
    prefix = {sset[0]: 1}
    for i, s in enumerate(sset[1:]):
        prefix[s] = 1 << (i + 1) * 10
    return int(num * prefix[letter])


def matplotlib_fixes():
    # force to find normal weight times new roman
    # this is necessary until matplotlib 3.2.0
    if 'roman' in mpl.font_manager.weight_dict:
        del mpl.font_manager.weight_dict['roman']
        mpl.font_manager._rebuild()


def cleanup_axis_timedelta(axis, formatter=None):
    if formatter is None:
        def formatter(x, pos):
            return '{:.1f}'.format(x)
    axis.set_major_locator(mticker.MaxNLocator(nbins=4, min_n_ticks=2))
    axis.set_major_formatter(mticker.FuncFormatter(formatter))
    return axis


def cleanup_axis_datetime(axis, rule=None):
    if rule is None:
        rule = rrulewrapper(SECONDLY, interval=1)
    loc = RRuleLocator(rule)
    fmt = DateFormatter('%M:%S')
    axis.set_major_locator(loc)
    axis.set_major_formatter(fmt)
    return axis


def cleanup_axis_categorical(axis, values):
    axis.set_major_formatter(mticker.FuncFormatter(lambda x, _: dict(zip(range(len(values)), values)).get(x, "")))
    

def cleanup_axis_percent(axis, **kwargs):
    axis.set_major_formatter(mticker.PercentFormatter(**kwargs))


def axhlines(ys, xmin=0, xmax=1, **kwargs):
    """
    Draw horizontal lines across plot
    :param ys: A scalar, list, or 1D array of y position in data coordinates
        of the horizontal line.
    :param xmin: A scalar, list, or 1D array. Should be between 0 and 1,
        0 being the far left of the plot, 1 the far right of the plot.
    :param xmax: A scalar, list, or 1D array. Should be between 0 and 1,
        0 being the far left of the plot, 1 the far right of the plot.
    :param kwargs: Valid kwargs are
        :class:`~matplotlib.collections.LineCollection` properties, with the
        exception of 'transform'.
    :return: The LineCollection object corresponding to the lines.
    """
    if "transform" in kwargs:
        raise ValueError("'transform' is not allowed as a kwarg;"
                         + "axhlines generates its own transform.")

    # prepare data
    ys = np.array((ys, ) if np.isscalar(ys) else ys, copy=False)
    xmins = np.array((xmin, ) if np.isscalar(xmin) else xmin, copy=False)
    xmaxs = np.array((xmax, ) if np.isscalar(xmax) else xmax, copy=False)

    if len(ys) > 1:
        if len(xmins) == 1:
            xmins = np.repeat(xmins, len(ys))
        if len(xmaxs) == 1:
            xmaxs = np.repeat(xmaxs, len(ys))

    if len(xmins) != len(xmaxs) or len(xmins) != len(ys):
        raise ValueError("Incompatible data")

    # prepare the ax
    ax = kwargs.pop('ax', None)
    if ax is None:
        ax = plt.gca()

    # prepare colors
    colors = kwargs.pop('colors', None)
    if colors is None:
        cycle_props = next(ax._get_lines.prop_cycler)
        colors = cycle_props.pop('color', None)

    # prepare trans
    trans = ax.get_yaxis_transform(which='grid')
    # prepare lines
    lines = [
        ([xmin, y], [xmax, y])
        for xmin, xmax, y in zip(xmins, xmaxs, ys)
    ]
    lc = LineCollection(lines, transform=trans, colors=colors, **kwargs)
    ax.add_collection(lc)
    ax.autoscale_view(scalex=False, scaley=True)

    return lc


def axvlines(xs, ymin=0, ymax=1, **kwargs):
    """
    Draw vertical lines across plot
    :param xs: A scalar, list, or 1D array of x position in data coordinates
        of the horizontal line.
    :param ymin: A scalar, list, or 1D array. Should be between 0 and 1,
        0 being the bottom of the plot, 1 the top of the plot.
    :param ymax: A scalar, list, or 1D array. Should be between 0 and 1,
        0 being the bottom of the plot, 1 the top of the plot.
    :param kwargs: Valid kwargs are
        :class:`~matplotlib.collections.LineCollection` properties, with the
        exception of 'transform'.
    :return: The LineCollection object corresponding to the lines.
    """
    if "transform" in kwargs:
        raise ValueError("'transform' is not allowed as a kwarg;"
                         + "axvlines generates its own transform.")

    # prepare data
    xs = np.array((xs, ) if np.isscalar(xs) else xs, copy=False)
    ymins = np.array((ymin, ) if np.isscalar(ymin) else ymin, copy=False)
    ymaxs = np.array((ymax, ) if np.isscalar(ymax) else ymax, copy=False)

    if len(xs) > 1:
        if len(ymins) == 1:
            ymins = np.repeat(ymins, len(xs))
        if len(ymaxs) == 1:
            ymaxs = np.repeat(ymaxs, len(xs))

    if len(ymins) != len(ymaxs) or len(ymins) != len(xs):
        raise ValueError("Incompatible data")

    # prepare the ax
    ax = kwargs.pop('ax', None)
    if ax is None:
        ax = plt.gca()

    # prepare colors
    colors = kwargs.pop('colors', None)
    if colors is None:
        cycle_props = next(ax._get_lines.prop_cycler)
        colors = cycle_props.pop('color', None)

    # prepare trans
    trans = ax.get_xaxis_transform(which='grid')
    # prepare lines
    lines = [
        ([x, ymin], [x, ymax])
        for x, ymin, ymax in zip(xs, ymins, ymaxs)
    ]
    lc = LineCollection(lines, transform=trans, colors=colors, **kwargs)
    ax.add_collection(lc)
    ax.autoscale_view(scalex=True, scaley=False)

    return lc


@contextmanager
def pbopen(filename):
    from tqdm import tqdm
    total = getsize(filename)
    pb = tqdm(total=total, unit="B", unit_scale=True,
              desc=basename(filename), miniters=1,
              ncols=80, ascii=True)

    def wrapped_line_iterator(fd):
        processed_bytes = 0
        for line in fd:
            processed_bytes += len(line)
            # update progress every MB.
            if processed_bytes >= 1024 * 1024:
                pb.update(processed_bytes)
                processed_bytes = 0

            yield line

        # finally
        pb.update(processed_bytes)
        pb.close()

    with open(filename) as fd:
        yield wrapped_line_iterator(fd)

def cdf(X, ax=None, **kws):
    if ax is None:
        _, ax = plt.subplots()
    n = np.arange(1,len(X)+1) / np.float(len(X))
    Xs = np.sort(X)
    ax.step(Xs, n, **kws)
    ax.set_ylim(0, 1)
    return ax


def bar(df, width=0.8, ax=None, **kwargs):
    '''Draw bars, the columns will be series, the index will be x-axis, the value will be y-axis '''
    if ax is None:
        ax = plt.gca()
    width = kwargs.pop('width', 0.8)

    nseries = len(df.columns)

    bar_width = width / nseries

    ind = np.arange(len(df))
    tick_ind = ind + bar_width * (nseries - 1) / 2

    cycle = ax._get_lines.prop_cycler

    for col, prop in zip(df.columns, cycle):
        ax.bar(ind, df[col], bar_width, label=col, **{**prop, **kwargs})
        ind = ind + bar_width

    ax.set_xticks(tick_ind)
    ax.set_xticklabels(df.index)
    ax.tick_params(axis='x', rotation=90)
    ax.legend()

    return ax


def bar_show_data(x, y, ax=None, data_y=None, fmt='{:.1f}', **kwargs):
    '''Show a single data point'''
    kws = {
        'xytext': [0, 7],
        'textcoords': 'offset points',
        'size': 7,
        'horizontalalignment': 'center',
        'verticalalignment': 'top'
    }
    if data_y is None:
        data_y = y
    if ax is None:
        ax = plt.gca()
    ax.annotate(fmt.format(data_y),
                xy=[x, y],
                **{**kws, **kwargs})


class DraggableLine:
    def __init__(self, ax, orientation, XorY, **kwargs):
        self.ax = ax
        self.c = ax.get_figure().canvas
        self.o = orientation
        self.XorY = XorY

        lineArgs = {
            'linestyle': '--',
            'picker': 5
        }
        lineArgs.update(kwargs)

        if orientation == "h":
            self.line = self.ax.axhline(self.XorY, **lineArgs)
        elif orientation == "v":
            self.line = self.ax.axvline(self.XorY, **lineArgs)
        else:
            assert False

        self.c.draw_idle()
        self.sid = self.c.mpl_connect('pick_event', self.onClick)

    def onClick(self, event):
        if event.artist == self.line:
            print("line selected ", event.artist)
            self.follower = self.c.mpl_connect("motion_notify_event", self.onMotion)
            self.releaser = self.c.mpl_connect("button_press_event", self.onRelease)

    def onMotion(self, event):
        if self.o == "h":
            self.XorY = self.line.get_ydata()[0]
            self.line.set_ydata([event.ydata, event.ydata])
        else:
            self.XorY = self.line.get_xdata()[0]
            self.line.set_xdata([event.xdata, event.xdata])
        self.c.draw_idle()

    def onRelease(self, event):
        if self.o == "h":
            self.XorY = self.line.get_ydata()[0]
        else:
            self.XorY = self.line.get_xdata()[0]

        print (self.XorY)

        self.c.mpl_disconnect(self.releaser)
        self.c.mpl_disconnect(self.follower)


def draggable_line(data, orientation='h', ax=None, **kwargs):
    if ax is None:
        ax = plt.gca()

    dline = DraggableLine(ax, orientation, data, **kwargs)
    return dline


def fig_legend(ax, **kwargs):
    handles, labels = ax.get_legend_handles_labels()
    return ax.figure.legend(handles, labels, **kwargs)


@contextmanager
def prepare_paper(nclz=None, styles=None, enable=True, props=None):
    if styles is None:
        styles = []
    matplotlib_fixes()
    
    all_styles = ['seaborn-paper', 'mypaper']
    all_styles += [
        {
            'axes.prop_cycle': (
                cycler(linestyle=['-', '-.', ':', '--']) * 3
                + cycler(color=['#d7191c', '#fdae61', '#000000', '#abdda4', '#2b83ba', '#91bfbd']) * 2
            )
        }
    ]
    all_styles += styles
        
    if not enable:
        yield
    else:
        with plt.style.context(all_styles):
            yield
        plt.gcf().canvas.draw()
        plt.close()

def save_paper_pdf(fig, path, width=3.15, height=3.15):
    fig.set_size_inches(width, height, forward=True)
    
    fig.savefig(path, dpi=300, bbox_inches='tight')
    

def np_mask_column_trail_zero(a, shrink=None):
    """Given a 2D array, return a mask of the same shape,
    with trailing zero entries as False, others as True
    """
    assert len(a.shape) == 2
    mask = np.full(a.shape, True)
    for col in range(a.shape[1]):
        # we want the last run for each column
        runs = pred_runs(a[:, col], a[:, col] == 0)
        if runs.shape[0] < 1:
            continue
        run = runs[-1, :]
        st, ed = run
        if ed != a.shape[0]:
            # there is not trialing zero
            continue
        if shrink is not None:
            st = ed - round((ed - st) * shrink)
        # update the mask
        mask[st:ed, col] = False
    return mask


def pred_runs(a, pred=None):
    """Given an 1D array, return
    n x 2 2D array giving [start, end) range of pred == True
    """
    if pred is None:
        pred = np.equal(a, 0)
    # Create an array that is 1 where a is 0, and pad each end with an extra 0.
    iszero = np.concatenate(([0], pred.view(np.int8), [0]))
    absdiff = np.abs(np.diff(iszero))
    # Runs start and end where absdiff is 1.
    ranges = np.where(absdiff == 1)[0].reshape(-1, 2)
    return ranges


def roundrobin(*iterables):
    "roundrobin('ABC', 'D', 'EF') --> A D E B F C"
    # Recipe credited to George Sakkis
    pending = len(iterables)
    nexts = itertools.cycle(iter(it).__next__ for it in iterables)
    while pending:
        try:
            for next in nexts:
                yield next()
        except StopIteration:
            pending -= 1
            nexts = itertools.cycle(itertools.islice(nexts, pending))
            

def job_timeline(workers, begin, end,
             groupby=None, label=None,
             ax=None,
             marker_begin=None, marker_end=None,
             markersize=None,
             group_num=2, group_radius=.3):
    '''Draw horizontal timelines of jobs
        Args:
            These are usually a column of a dataframe
            
            workers: list
            begin: list
            end: list
            groupby: list
            
            The following two controls small offset in y_pos, to create a wave like shape
            
            group_num: int
            group_radius: float
    '''
    if marker_begin is None:
        marker_begin = default_marker_begin()
    if marker_end is None:
        marker_end = default_marker_end()

    if int(group_num) <= 0:
        raise ValueError(f'group_num should be a positive integer, but got {group_num}')
    group_num = int(group_num)

    if groupby is None:
        if not len(workers) == len(begin) == len(end):
            raise ValueError('Length of workers, begin, end should be equal,'
                             f' but got ({len(workers)}, {len(begin)}, {len(end)})')
    else:
        if not isinstance(groupby, list):
            groupby = [groupby]

        lens = [len(col) for col in [workers, begin, end] + groupby]
        if not check_equal(lens):
            raise ValueError('Length of workers, begin, end, and col in groupby should be equal,'
                             f' but got {lens}')

    # create y_pos according to workers, so workers doesn't has to be numeric
    y_values, y_pos = np.unique(workers, return_inverse=True)
    y_pos = y_pos.astype(np.float64)

    # adjust y_pos according to a wave like shape around original y_pos,
    # the offset should be changing based on the index within a particular y_value
    offset_pattern = np.concatenate([
        np.arange(0, group_num),
        np.arange(group_num, -group_num, step=-1),
        np.arange(-group_num, 0)
    ])
    for worker in y_values:
        mask = workers == worker
        num = len(workers[mask])
        offset = np.tile(offset_pattern, (num + len(offset_pattern) - 1) // len(offset_pattern))[:num]
        y_pos[mask] += offset * group_radius / group_num

    if ax is None:
        _, ax = plt.subplots()

    def draw_group(y, xmin, xmax, c, key=None):
        # label
        if key is None:
            theLabel = label
        else:
            theLabel = (label or '{key}').format(key=key) if key is not None else label
        # draw lines
        ax.hlines(y, xmin, xmax, label=theLabel, color=c)
        # draw markers
        ax.plot(xmin, y, color=c,
                marker=marker_begin, markersize=markersize,
                linestyle='None', fillstyle='none')
        ax.plot(xmax, y, color=c,
                marker=marker_end, markersize=markersize,
                linestyle='None', fillstyle='none')

    if groupby is None:
        c = next(ax._get_lines.prop_cycler)['color']
        draw_group(y_pos, begin, end, c)
    else:
        if len(groupby) >= 1 and len(groupby) <= 2:
            # cycle color
            c = next(ax._get_lines.prop_cycler)['color']
            colors = {}
            for grp_key, (y, xmin, xmax) in gen_groupby(y_pos, begin, end, groups=groupby):
                if grp_key[0] not in colors:
                    colors[grp_key[0]] = next(ax._get_lines.prop_cycler)['color']
                c = colors[grp_key[0]]
                if len(grp_key) >= 2:
                    c = adjust_lightness(c, 1.5 - grp_key[1] * 0.3)
                draw_group(y, xmin, xmax, c, key=grp_key)
        else:
            raise ValueError('Unsupported groupby')

    # fix yticks to categorical
    cleanup_axis_categorical(ax.yaxis, y_values)

    # set a default title
    ax.set_ylabel('Worker')
    ax.set_xlabel('Time')

    return ax


def adjust_lightness(color, amount=0.5):
    '''
    the color gets brighter when amount > 1 and darker when amount < 1
    '''
    import matplotlib.colors as mc
    import colorsys
    try:
        c = mc.cnames[color]
    except KeyError:
        c = color
    c = colorsys.rgb_to_hls(*mc.to_rgb(c))
    return colorsys.hls_to_rgb(c[0], max(0, min(1, amount * c[1])), c[2])
