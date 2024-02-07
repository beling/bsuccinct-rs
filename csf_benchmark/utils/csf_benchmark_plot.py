import pandas as pd
import matplotlib.pyplot as plt
from sys import stderr

data_dir = "csf_benchmark_results"

def get_data(filename):
    '''Try to read data from file and returns them or None.'''
    try:
        d = pd.read_csv(f"{data_dir}/{filename}.csv", sep=' ')
        if d.empty:
            print(f"no data {filename}.csv")
            return None
        return d
    except FileNotFoundError:
        print(f"{filename}.csv not found and thus skipped", file=stderr)
        return None

UNCOMPRESSED_LABEL = '(uncompressed) vector of values'

def add_to_entropy_plot(filename, as_percent=False, label=None, params={}, conf={}):
    '''Add to plot entropy->overhead data from given file.'''
    d = get_data(filename)
    if d is None: return
    if label is None: label = filename
    for f, v in params.items():
        d = d[d[f] == v]
        label += f' {f}={v}'
    #best_idx = d.groupby(['input_len', 'entropy'])['bits/entry'].transform(min) == d['bits/entry']
    #print(d[best_idx])
    if conf.get('uncompressed_plot', False):
        d[UNCOMPRESSED_LABEL] = d.apply(lambda row: int(row.different_values-1).bit_length() - row.entropy, axis=1)
        if as_percent: d[UNCOMPRESSED_LABEL] = 100 * d[UNCOMPRESSED_LABEL] / d['entropy']
        d2 = d.groupby(['entropy'])[UNCOMPRESSED_LABEL].min()
        d2.plot(legend=True, style='k--')   # https://matplotlib.org/stable/users/explain/colors/colors.html
        conf['uncompressed_plot'] = False
    if as_percent:
        d[label] = 100 * (d['bits/entry'] - d['entropy']) / d['entropy']
    else:
        d[label] = d['bits/entry'] - d['entropy']
    d = d.groupby(['entropy'])[label].min() # get entropy -> result for best configuration table
    d.plot(legend=True)


def add_to_uncompressed_plot(filename, as_percent=False, label=None):
    '''Add to plot bits_per_uncompressed_entry->overhead data from given file.'''
    d = get_data(filename)
    if d is None: return
    if label is None: label = filename
    d['input_size'] = d.apply(lambda row: int(row.input_len) * int(row.different_values-1).bit_length(), axis=1)
    d['sf_size'] = d['input_len'] * d['bits/entry']
    print(d['input_size'])
    print(d['sf_size'])
    if as_percent:
        d[label] = 100 * (d['sf_size'] - d['input_size']) / d['input_size']
    else:
        d[label] = d['sf_size'] - d['input_size']
    d = d.groupby(['input_size'])[label].min() # get bit_length -> result for best configuration table
    d.plot(legend=True)

def show_entropy_plot(as_percent=False, dataset='equal', conf={}):
    if as_percent: plt.ylim(0, 300)
    add_to_entropy_plot(f'fp_{dataset}', as_percent=as_percent, label='fp', conf=conf)
    add_to_entropy_plot(f'fpgo_{dataset}', as_percent=as_percent, params={'bits/seed': 4, 'bits/group': 16, 'level_size': 80}, label='fpgo', conf=conf)
    add_to_entropy_plot(f'fpgo_{dataset}', as_percent=as_percent, params={'bits/seed': 8, 'bits/group': 32, 'level_size': 100}, label='fpgo', conf=conf)
    add_to_entropy_plot(f'ls_{dataset}', as_percent=as_percent, label='ls', conf=conf)
    plt.ylabel('overhead/entropy [%]' if as_percent else 'overhead [bits/key]')
    plt.show()

def show_uncompressed_plot(as_percent=False):
    add_to_uncompressed_plot('fp_equal', as_percent=as_percent, label='fp')
    add_to_uncompressed_plot('ls_equal', as_percent=as_percent, label='ls')
    #add_to_uncompressed_plot('fp_dominated', as_percent=as_percent)
    #add_to_uncompressed_plot('ls_dominated', as_percent=as_percent)
    plt.ylabel('overhead/input_size [%]' if as_percent else 'overhead [bits]')
    plt.show()

show_entropy_plot(False, 'equal', {'uncompressed_plot': True})
show_entropy_plot(True, 'equal', {'uncompressed_plot': True})
show_entropy_plot(False, 'dominated', {'uncompressed_plot': True})
show_entropy_plot(True, 'dominated', {'uncompressed_plot': True})
#show_uncompressed_plot(False)
#show_uncompressed_plot(True)