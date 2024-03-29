import pandas as pd
import matplotlib.pyplot as plt
from sys import stderr
import subprocess

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

def show_entropy_plot(as_percent=False, dataset='equal', compressed_methods=True, conf={}):
    plt.title('{} space overhead for {} distribution'.format("Relative" if as_percent else "Absolute", dataset))
    plt.xlim(0, 8)
    if compressed_methods:
        add_to_entropy_plot(f'cfp_{dataset}', as_percent=as_percent, label='fp::CMap', conf=conf)
        add_to_entropy_plot(f'cls_{dataset}', as_percent=as_percent, label='ls::CMap', conf=conf)
        #add_to_entropy_plot(f'cfpgo_{dataset}', as_percent=as_percent, params={'bits/seed': 4, 'bits/group': 16, 'level_size_percent': 80}, label='fp::CMap', conf=conf)
        add_to_entropy_plot(f'cfpgo_{dataset}', as_percent=as_percent, params={'bits/seed': 1}, label='fp::GOCMap', conf=conf)
        add_to_entropy_plot(f'cfpgo_{dataset}', as_percent=as_percent, params={'bits/seed': 2}, label='fp::GOCMap', conf=conf)
        add_to_entropy_plot(f'cfpgo_{dataset}', as_percent=as_percent, params={'bits/seed': 4}, label='fp::GOCMap', conf=conf)
        add_to_entropy_plot(f'cfpgo_{dataset}', as_percent=as_percent, params={'bits/seed': 8}, label='fp::GOCMap', conf=conf)
    else:
        add_to_entropy_plot(f'fp_{dataset}', as_percent=as_percent, label='fp::Map', conf=conf)    
        add_to_entropy_plot(f'ls_{dataset}', as_percent=as_percent, label='ls::Map', conf=conf)
    plt.xlabel('entropy of the distribution of function values')
    plt.ylabel('overhead/entropy [%]' if as_percent else 'overhead [bits/key]')
    if as_percent: plt.ylim(0, 300)
    else: plt.ylim(bottom=0)
    svg_name = "{}{}_{}".format(dataset,'_comp' if compressed_methods else '', 'rel' if as_percent else 'abs')
    plt.savefig(f"uncompressed_{svg_name}.svg", facecolor=(1.0, 1.0, 1.0, 0.8))
    subprocess.run(["scour", "-i", f"uncompressed_{svg_name}.svg", "-o", f"{svg_name}.svg"])
    #, "--enable-viewboxing", "--enable-id-stripping", "--enable-comment-stripping", "--shorten-ids", "--indent=none"
    #subprocess.run(["minify", "-o", f"{svg_name}.svg", f"uncompressed_{svg_name}.svg"])
    plt.show()

def show_uncompressed_plot(as_percent=False):
    add_to_uncompressed_plot('fp_equal', as_percent=as_percent, label='fp::Map')
    add_to_uncompressed_plot('ls_equal', as_percent=as_percent, label='ls::Map')
    #add_to_uncompressed_plot('fp_dominated', as_percent=as_percent, label='fp::Map')
    #add_to_uncompressed_plot('ls_dominated', as_percent=as_percent, label='ls::Map')
    plt.ylabel('overhead/input_size [%]' if as_percent else 'overhead [bits]')
    plt.show()

for compressed_methods in (False, True):
    show_entropy_plot(False, 'equal', compressed_methods, {'uncompressed_plot': True})
    show_entropy_plot(True, 'equal', compressed_methods, {'uncompressed_plot': True})
    show_entropy_plot(False, 'dominated', compressed_methods, {'uncompressed_plot': True})
    show_entropy_plot(True, 'dominated', compressed_methods, {'uncompressed_plot': True})
#show_uncompressed_plot(False)
#show_uncompressed_plot(True)