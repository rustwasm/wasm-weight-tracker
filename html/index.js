let absolute = true;

const series = {};

async function run() {
  const response = await fetch('./data.json');
  const json = await response.json();

  for (let day of json) {
    // dates[day.date] = day.data;
    for (let bm of day.data) {
      if (series[bm.name] === undefined)
        series[bm.name] = {};
      const data = series[bm.name];

      for (let output of bm.outputs) {
        if (data[output.name] === undefined)
          data[output.name] = [];
        data[output.name].push([Date.parse(day.date), output.bytes]);
      }
      // if (names[bm.name] === undefined)
      //   names[bm.name] = {};
      // names[bm.name][day.date] = bm;
    }
  }

  const body = document.body;
  for (let bm in series) {
    const element = document.createElement('div');
    element.id = bm;
    body.appendChild(element);
  }

  const button = document.getElementById('absolute');
  absolute = button.checked;
  button.onclick = function() {
    absolute = button.checked;
    render();
  };
  document.getElementById('controls').style.display = 'block';

  render();

  document.getElementById('loading').remove();
}

function render() {
  for (let bm in series) {
    const data = [];
    for (let output in series[bm]) {
      let sizes = series[bm][output];
      let raw = [];
      for (let datapoint of sizes) {
        let value = 0;
        if (absolute) {
          value = datapoint[1];
        } else {
          value = datapoint[1] / sizes[0][1];
        }
        raw.push([datapoint[0], value]);
      }
      data.push({ name: output, data: raw });
    }

    var myChart = Highcharts.chart(bm, {
      title: {
        text: bm,
      },
      yAxis: {
        title: {
          text: 'Size (in bytes)',
        },
        labels: {
          formatter: function() { return format(this.value); },
        },
      },
      xAxis: {
        type: 'datetime',
        dateTimeLabelFormats: {
          month: '%e. %b',
          year: '%b',
        },
        title: {
          text: 'Date',
        },
      },
      tooltip: {
        headerFormat: '<b>{series.name}</b><br>',
        pointFormatter: function() { return format(this.y); },
      },
      series: data,
    });
  }
}

function format(val) {
  if (absolute) {
    return bytes(val);
  }

  const pct = Math.round(val * 10000) / 100;
  return `${pct}%`;
}

function bytes(val) {
  if (val < 1024) {
    return `${val}B`;
  }
  val = Math.round((val * 100) / 1024) / 100;
  if (val < 1024) {
    return `${val}KB`;
  }
  val = Math.round((val * 100) / 1024) / 100;
  if (val < 1024) {
    return `${val}MB`;
  }
  val = Math.round((val * 100) / 1024) / 100;
  return `${val}GB`;
}

document.addEventListener('DOMContentLoaded', run);
